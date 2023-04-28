use anyhow::{anyhow, Result};
use cid::Cid;
use globset::{Glob, GlobSet, GlobSetBuilder};
use noosphere_core::{
    authority::Authorization,
    data::{BodyChunkIpld, ContentType, Did, Header},
    view::Sphere,
};
use noosphere_storage::{BlockStore, KeyValueStore, NativeStorage, SphereDb, Store};
use pathdiff::diff_paths;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use subtext::util::to_slug;
use tokio::fs::{self, File};
use tokio::io::copy;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio_stream::StreamExt;
use ucan_key_support::ed25519::Ed25519KeyMaterial;
use url::Url;

use noosphere::{
    key::{InsecureKeyStorage, KeyStorage},
    sphere::SphereContextBuilder,
};

use noosphere_sphere::{
    HasSphereContext, SphereContentRead, SphereContext, AUTHORIZATION, GATEWAY_URL, USER_KEY_NAME,
};

use tempfile::TempDir;

const SPHERE_DIRECTORY: &str = ".sphere";
const NOOSPHERE_DIRECTORY: &str = ".noosphere";

pub type CliSphereContext = SphereContext<Ed25519KeyMaterial, NativeStorage>;

/// A delta manifest of changes to the local content space
#[derive(Default)]
pub struct ContentChanges {
    pub new: BTreeMap<String, Option<ContentType>>,
    pub updated: BTreeMap<String, Option<ContentType>>,
    pub removed: BTreeMap<String, Option<ContentType>>,
    pub unchanged: BTreeSet<String>,
}

impl ContentChanges {
    pub fn is_empty(&self) -> bool {
        self.new.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

/// A manifest of content to apply some work to in the local content space
#[derive(Default)]
pub struct Content {
    pub matched: BTreeMap<String, FileReference>,
    pub ignored: BTreeSet<String>,
}

impl Content {
    pub fn is_empty(&self) -> bool {
        self.matched.is_empty()
    }
}

/// Metadata that identifies some sphere content that is present on the file
/// system
pub struct FileReference {
    pub cid: Cid,
    pub content_type: ContentType,
    pub extension: Option<String>,
}

use super::commands::config::COUNTERPART;

/// A [Workspace] represents the root directory where sphere data is or will be
/// kept, and exposes core operations that commands need in order to operate on
/// that directory. Among other things, it holds a singleton [SphereContext] so
/// that we don't constantly open the local [SphereDb] multiple times in the
/// space of a single command. It also offers a convenient entrypoint to access
/// [KeyStorage] for the local platform.
pub struct Workspace {
    root_directory: PathBuf,
    sphere_directory: PathBuf,
    // storage_directory: PathBuf,
    key_storage: InsecureKeyStorage,
    sphere_context: OnceCell<Arc<Mutex<CliSphereContext>>>,
}

impl Workspace {
    /// Get a mutex-guarded reference to the [SphereContext] for the current workspace
    pub async fn sphere_context(&self) -> Result<Arc<Mutex<CliSphereContext>>> {
        Ok(self
            .sphere_context
            .get_or_try_init(|| async {
                Ok(Arc::new(Mutex::new(
                    SphereContextBuilder::default()
                        .open_sphere(None)
                        .at_storage_path(&self.root_directory)
                        .reading_keys_from(self.key_storage.clone())
                        .build()
                        .await?
                        .into(),
                ))) as Result<Arc<Mutex<CliSphereContext>>, anyhow::Error>
            })
            .await?
            .clone())
    }

    /// Get an owned referenced to the [SphereDb] that backs the local sphere.
    /// Note that this will initialize the [SphereContext] if it has not been
    /// already.
    pub async fn db(&self) -> Result<SphereDb<NativeStorage>> {
        let context = self.sphere_context().await?;
        let context = context.lock().await;
        Ok(context.db().clone())
    }

    /// Get the [KeyStorage] that is supported on the current platform
    pub fn key_storage(&self) -> &InsecureKeyStorage {
        &self.key_storage
    }

    /// The root directory is the path to the folder that contains a .sphere
    /// directory (or will, after the sphere is initialized)
    pub fn root_directory(&self) -> &Path {
        &self.root_directory
    }

    /// The path to the .sphere directory within the root directory
    pub fn sphere_directory(&self) -> &Path {
        &self.sphere_directory
    }

    /// This directory contains keys that are stored using an insecure, strait-
    /// to-disk storage mechanism
    pub fn key_directory(&self) -> &Path {
        self.key_storage().storage_path()
    }

    /// Gets the [Did] of the sphere
    pub async fn sphere_identity(&self) -> Result<Did> {
        let context = self.sphere_context().await?;
        let context = context.lock().await;

        Ok(context.identity().clone())
    }

    pub fn ensure_sphere_initialized(&self) -> Result<()> {
        match self.sphere_directory().exists() {
            false => Err(anyhow!(
                "No sphere initialized in {:?}",
                self.root_directory()
            )),
            true => Ok(()),
        }
    }

    pub fn ensure_sphere_uninitialized(&self) -> Result<()> {
        match self.sphere_directory().exists() {
            true => Err(anyhow!(
                "A sphere is already initialized in {:?}",
                self.root_directory()
            )),
            false => Ok(()),
        }
    }

    /// Returns true if the given path has a .sphere folder in it
    fn has_sphere_directory(path: &Path) -> bool {
        path.is_absolute() && path.join(SPHERE_DIRECTORY).is_dir()
    }

    /// Returns a [PathBuf] pointing to the nearest ancestor directory that
    /// contains a .sphere directory, if one exists
    fn find_root_directory(from: Option<&Path>) -> Option<PathBuf> {
        debug!("Looking for .sphere in {:?}", from);

        match from {
            Some(directory) => {
                if Workspace::has_sphere_directory(directory) {
                    Some(directory.into())
                } else {
                    Workspace::find_root_directory(directory.parent())
                }
            }
            None => None,
        }
    }

    /// Produces a manifest of changes (added, updated and removed) derived from
    /// the current state of the workspace
    pub async fn get_file_content_changes<S: Store>(
        &self,
        new_blocks: &mut S,
    ) -> Result<Option<(Content, ContentChanges)>> {
        let db = self.db().await?;
        let sphere_context = self.sphere_context().await?;
        let sphere_cid = sphere_context.version().await?;

        let file_content = self.read_file_content(new_blocks).await?;

        let sphere = Sphere::at(&sphere_cid, &db);
        let content = sphere.get_content().await?;

        let mut stream = content.stream().await?;

        let mut changes = ContentChanges::default();

        while let Some(Ok((slug, memo))) = stream.next().await {
            if file_content.ignored.contains(slug) {
                continue;
            }

            match file_content.matched.get(slug) {
                Some(FileReference {
                    cid: body_cid,
                    content_type,
                    extension: _,
                }) => {
                    let sphere_file = sphere_context.read(slug).await?.ok_or_else(|| {
                        anyhow!(
                            "Expected sphere file at slug {:?} but it was missing!",
                            slug
                        )
                    })?;

                    if &sphere_file.memo.body == body_cid {
                        changes.unchanged.insert(slug.clone());
                        continue;
                    }

                    changes
                        .updated
                        .insert(slug.clone(), Some(content_type.clone()));
                }
                None => {
                    let memo = memo.load_from(&db).await?;
                    changes.removed.insert(slug.clone(), memo.content_type());
                }
            }
        }

        for (slug, FileReference { content_type, .. }) in &file_content.matched {
            if changes.updated.contains_key(slug)
                || changes.removed.contains_key(slug)
                || changes.unchanged.contains(slug)
            {
                continue;
            }

            changes.new.insert(slug.clone(), Some(content_type.clone()));
        }

        Ok(Some((file_content, changes)))
    }

    /// Read the local content of the workspace in its entirety.
    /// This includes files that have not yet been saved to the sphere. All
    /// files are chunked into blocks, and those blocks are persisted to the
    /// provided store.
    /// TODO(#105): We may want to change this to take an optional list of paths to
    /// consider, and allow the user to rely on their shell for glob filtering
    pub async fn read_file_content<S: BlockStore>(&self, store: &mut S) -> Result<Content> {
        let root_path = &self.root_directory;
        let mut directories = vec![(None, tokio::fs::read_dir(root_path).await?)];

        let ignore_patterns = self.get_ignored_patterns().await?;

        let mut content = Content::default();

        while let Some((slug_prefix, mut directory)) = directories.pop() {
            while let Some(entry) = directory.next_entry().await? {
                let path = entry.path();
                let relative_path = diff_paths(&path, root_path)
                    .ok_or_else(|| anyhow!("Could not determine relative path to {:?}", path))?;

                if ignore_patterns.is_match(&relative_path) {
                    continue;
                }

                if path.is_dir() {
                    let slug_prefix = relative_path.to_string_lossy().to_string();

                    directories.push((Some(slug_prefix), tokio::fs::read_dir(path).await?));

                    // TODO: Limit the depth of the directory traversal to some reasonable number

                    continue;
                }

                let ignored = false;

                let name = match path.file_stem() {
                    Some(name) => name.to_string_lossy(),
                    None => continue,
                };

                let name = match &slug_prefix {
                    Some(prefix) => format!("{prefix}/{name}"),
                    None => name.to_string(),
                };

                let slug = match to_slug(&name) {
                    Ok(slug) if slug == name => slug,
                    _ => continue,
                };

                if ignored {
                    content.ignored.insert(slug);
                    continue;
                }

                let extension = path
                    .extension()
                    .map(|extension| String::from(extension.to_string_lossy()));

                let content_type = match &extension {
                    Some(extension) => self.infer_content_type(extension).await?,
                    None => ContentType::Bytes,
                };

                let file_bytes = fs::read(path).await?;
                let body_cid = BodyChunkIpld::store_bytes(&file_bytes, store).await?;

                content.matched.insert(
                    slug,
                    FileReference {
                        cid: body_cid,
                        content_type,
                        extension,
                    },
                );
            }
        }
        Ok(content)
    }

    /// Reads the latest local version of the sphere and renders its contents to
    /// files in the workspace. Note that this will overwrite any existing files
    /// in the workspace.
    pub async fn render(&self) -> Result<()> {
        let context = self.sphere_context().await?;
        let sphere = context.to_sphere().await?;

        let content = sphere.get_content().await?;

        let mut stream = content.stream().await?;

        // TODO(#106): We render the whole sphere every time, but we should probably
        // have a fast path where we only render the changes within a CID range
        while let Some(Ok((slug, _cid))) = stream.next().await {
            debug!("Rendering {}...", slug);

            let mut sphere_file = match context.read(slug).await? {
                Some(file) => file,
                None => {
                    warn!("Could not resolve content for {slug}");
                    continue;
                }
            };

            let extension = match sphere_file
                .memo
                .get_first_header(&Header::FileExtension.to_string())
            {
                Some(extension) => Some(extension),
                None => match sphere_file.memo.content_type() {
                    Some(content_type) => self.infer_file_extension(content_type).await,
                    None => {
                        warn!("No content type specified for {slug}; it will be rendered without a file extension");
                        None
                    }
                },
            };

            let file_fragment = match extension {
                Some(extension) => [slug.as_str(), &extension].join("."),
                None => slug.into(),
            };

            let file_path = self.root_directory.join(file_fragment);

            let file_directory = file_path
                .parent()
                .ok_or_else(|| anyhow!("Unable to determine root directory for {}", slug))?;

            fs::create_dir_all(&file_directory).await?;

            let mut fs_file = File::create(file_path).await?;

            copy(&mut sphere_file.contents, &mut fs_file).await?;
        }

        Ok(())
    }

    /// Produce a matcher that will match any path that should be ignored when
    /// considering the files that make up the local workspace
    async fn get_ignored_patterns(&self) -> Result<GlobSet> {
        // TODO(#82): User-specified ignore patterns
        let ignored_patterns = vec!["@*", ".*"];

        let mut builder = GlobSetBuilder::new();

        for pattern in ignored_patterns {
            builder.add(Glob::new(pattern)?);
        }

        Ok(builder.build()?)
    }

    /// Given a file extension, infer its mime
    pub async fn infer_content_type(&self, extension: &str) -> Result<ContentType> {
        // TODO: User-specified extension->mime mapping
        Ok(match extension {
            "subtext" => ContentType::Subtext,
            "sphere" => ContentType::Sphere,
            _ => ContentType::from_str(
                mime_guess::from_ext(extension)
                    .first_raw()
                    .unwrap_or("raw/bytes"),
            )?,
        })
    }

    /// Given a mime, infer its file extension
    pub async fn infer_file_extension(&self, content_type: ContentType) -> Option<String> {
        match content_type {
            ContentType::Subtext => Some("subtext".into()),
            ContentType::Sphere => Some("sphere".into()),
            ContentType::Bytes => None,
            ContentType::Unknown(content_type) => {
                match mime_guess::get_mime_extensions_str(&content_type) {
                    Some(extensions) => extensions.first().map(|str| String::from(*str)),
                    None => None,
                }
            }
            ContentType::Cbor => Some("json".into()),
            ContentType::Json => Some("cbor".into()),
        }
    }

    /// Get the key material (with both verification and signing capabilities)
    /// for the locally configured author key.
    pub async fn key(&self) -> Result<Ed25519KeyMaterial> {
        let key_name: String = self.db().await?.require_key(USER_KEY_NAME).await?;

        self.key_storage().require_key(&key_name).await
    }

    /// Get the configured counterpart sphere's identity
    pub async fn counterpart_identity(&self) -> Result<Did> {
        self.db().await?.require_key(COUNTERPART).await
    }

    /// Attempts to read the locally stored authorization that enables the key
    /// to operate on this sphere; the returned authorization may be represented
    /// as either a UCAN or the CID of a UCAN
    pub async fn authorization(&self) -> Result<Authorization> {
        Ok(self
            .db()
            .await?
            .require_key::<_, Cid>(AUTHORIZATION)
            .await?
            .into())
    }

    /// Get the configured gateway URL for the local workspace
    pub async fn gateway_url(&self) -> Result<Url> {
        self.db().await?.require_key(GATEWAY_URL).await
    }

    pub fn new(
        current_working_directory: &Path,
        noosphere_directory: Option<&Path>,
    ) -> Result<Self> {
        let root_directory = match Workspace::find_root_directory(Some(current_working_directory)) {
            Some(directory) => directory,
            None => current_working_directory.into(),
        };

        let noosphere_directory = match noosphere_directory {
            Some(custom_root) => custom_root.into(),
            None => home::home_dir()
                .ok_or_else(|| {
                    anyhow!(
                        "Could not discover home directory for {}",
                        whoami::username()
                    )
                })?
                .join(NOOSPHERE_DIRECTORY),
        };

        let key_storage = InsecureKeyStorage::new(&noosphere_directory)?;
        let sphere_directory = root_directory.join(SPHERE_DIRECTORY);

        Ok(Workspace {
            root_directory,
            sphere_directory,
            key_storage,
            sphere_context: OnceCell::new(),
        })
    }

    /// Configure a workspace automatically by creating temporary directories
    /// on the file system and initializing it with their paths
    pub fn temporary() -> Result<(Self, (TempDir, TempDir))> {
        let root = TempDir::new()?;
        let global_root = TempDir::new()?;

        Ok((
            Workspace::new(root.path(), Some(global_root.path()))?,
            (root, global_root),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::native::commands::{key, sphere};
    use tokio::fs;

    use super::Workspace;

    #[tokio::test]
    async fn it_chooses_an_ancestor_sphere_directory_as_root_if_one_exists() {
        let (workspace, _temporary_directories) = Workspace::temporary().unwrap();

        key::key_create("FOO", &workspace).await.unwrap();

        sphere::sphere_create("FOO", &workspace).await.unwrap();

        let subdirectory = workspace.root_directory().join("foo/bar");

        fs::create_dir_all(&subdirectory).await.unwrap();

        let new_workspace =
            Workspace::new(&subdirectory, workspace.key_directory().parent()).unwrap();

        assert_eq!(workspace.root_directory(), new_workspace.root_directory());
    }
}
