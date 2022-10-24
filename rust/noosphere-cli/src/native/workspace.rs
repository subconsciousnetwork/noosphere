use anyhow::{anyhow, Result};
use cid::Cid;
use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};
use libipld_cbor::DagCborCodec;
use noosphere::{
    authority::{restore_ed25519_key, Authorization},
    data::{BodyChunkIpld, ContentType, Header, MemoIpld},
    view::Sphere,
};
use noosphere_fs::SphereFs;
use noosphere_storage::{
    db::SphereDb,
    interface::{BlockStore, Store},
    native::{NativeStorageInit, NativeStorageProvider, NativeStore},
};
use path_absolutize::Absolutize;
use pathdiff::diff_paths;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};
use subtext::util::to_slug;
use tokio::{
    fs::{self, File},
    io::copy,
};
use tokio_stream::StreamExt;

use ucan_key_support::ed25519::Ed25519KeyMaterial;
use url::Url;

use super::commands::config::{Config, ConfigContents};

const NOOSPHERE_DIRECTORY: &str = ".noosphere";
const SPHERE_DIRECTORY: &str = ".sphere";
const BLOCKS_DIRECTORY: &str = "blocks";
const KEYS_DIRECTORY: &str = "keys";
const AUTHORIZATION_FILE: &str = "AUTHORIZATION";
const KEY_FILE: &str = "KEY";
const IDENTITY_FILE: &str = "IDENTITY";
const CONFIG_FILE: &str = "config.toml";

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

pub struct FileReference {
    pub cid: Cid,
    pub content_type: ContentType,
    pub extension: Option<String>,
}

/// A utility for discovering and initializing the well-known paths for a
/// working copy of a sphere and relevant global Noosphere configuration
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    sphere: PathBuf,
    blocks: PathBuf,
    noosphere: PathBuf,
    keys: PathBuf,
    authorization: PathBuf,
    key: PathBuf,
    identity: PathBuf,
    config: PathBuf,
}

impl Workspace {
    /// Read the local content of the workspace in its entirety, filtered by an
    /// optional glob pattern. The glob pattern is applied to the file path
    /// relative to the workspace. This includes files that have not yet been
    /// saved to the sphere. All files are chunked into blocks, and those blocks
    /// are persisted to the provided store.
    /// TODO(#105): We may want to change this to take an optional list of paths to
    /// consider, and allow the user to rely on their shell for glob filtering
    pub async fn read_local_content<S: BlockStore>(
        &self,
        pattern: Option<GlobMatcher>,
        store: &mut S,
    ) -> Result<Content> {
        self.expect_local_directories()?;

        let root_path = self.root_path();
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

                let mut ignored = false;

                // Ignore files that don't match an optional pattern
                if let Some(pattern) = &pattern {
                    if !pattern.is_match(&relative_path) {
                        ignored = true;
                    }
                }

                let name = match path.file_stem() {
                    Some(name) => name.to_string_lossy(),
                    None => continue,
                };

                let name = match &slug_prefix {
                    Some(prefix) => format!("{}/{}", prefix, name),
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

    /// Produces a manifest of changes (added, updated and removed) derived from
    /// the current state of the workspace
    pub async fn get_local_content_changes<Sa: Store, Sb: Store>(
        &self,
        pattern: Option<GlobMatcher>,
        db: &SphereDb<Sa>,
        new_blocks: &mut Sb,
    ) -> Result<Option<(Content, ContentChanges)>> {
        let sphere_did = self.get_local_identity().await?;
        let sphere_cid = match db.get_version(&sphere_did).await? {
            Some(cid) => cid,
            None => {
                return Ok(None);
            }
        };

        let content = self.read_local_content(pattern, new_blocks).await?;

        let sphere_fs = SphereFs::at(&sphere_did, &sphere_cid, None, db);
        let sphere = Sphere::at(&sphere_cid, db);
        let links = sphere.try_get_links().await?;

        let mut stream = links.stream().await?;

        let mut changes = ContentChanges::default();

        while let Some(Ok((slug, cid))) = stream.next().await {
            if content.ignored.contains(slug) {
                continue;
            }

            match content.matched.get(slug) {
                Some(FileReference {
                    cid: body_cid,
                    content_type,
                    extension: _,
                }) => {
                    let sphere_file = sphere_fs.read(slug).await?.ok_or_else(|| {
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
                    let memo = db.load::<DagCborCodec, MemoIpld>(cid).await?;

                    changes.removed.insert(slug.clone(), memo.content_type());
                }
            }
        }

        for (slug, FileReference { content_type, .. }) in &content.matched {
            if changes.updated.contains_key(slug)
                || changes.removed.contains_key(slug)
                || changes.unchanged.contains(slug)
            {
                continue;
            }

            changes.new.insert(slug.clone(), Some(content_type.clone()));
        }

        Ok(Some((content, changes)))
    }

    /// Reads the latest local version of the sphere and renders its contents to
    /// files in the workspace. Note that this will overwrite any existing files
    /// in the workspace.
    pub async fn render<S: Store>(&self, db: &mut SphereDb<S>) -> Result<()> {
        let sphere_did = self.get_local_identity().await?;
        let sphere_cid = db.require_version(&sphere_did).await?;
        let sphere_fs = SphereFs::at(&sphere_did, &sphere_cid, None, db);
        let sphere = Sphere::at(&sphere_cid, db);
        let links = sphere.try_get_links().await?;

        let mut stream = links.stream().await?;

        // TODO(#106): We render the whole sphere every time, but we should probably
        // have a fast path where we only render the changes within a CID range
        while let Some(Ok((slug, _cid))) = stream.next().await {
            debug!("Rendering {}...", slug);

            let mut sphere_file = match sphere_fs.read(slug).await? {
                Some(file) => file,
                None => {
                    println!("Warning: could not resolve content for {}", slug);
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
                        println!("Warning: no content type specified for {}; it will be rendered without a file extension", slug);
                        None
                    }
                },
            };

            let file_fragment = match extension {
                Some(extension) => [slug.as_str(), &extension].join("."),
                None => slug.into(),
            };

            let file_path = self.root.join(file_fragment);

            let file_directory = file_path
                .parent()
                .ok_or_else(|| anyhow!("Unable to determine root directory for {}", slug))?;

            fs::create_dir_all(&file_directory).await?;

            let mut fs_file = File::create(file_path).await?;

            copy(&mut sphere_file.contents, &mut fs_file).await?;
        }

        Ok(())
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
        }
    }

    /// Produce a matcher that will match any path that should be ignored when
    /// considering the files that make up the local workspace
    pub async fn get_ignored_patterns(&self) -> Result<GlobSet> {
        self.expect_local_directories()?;

        // TODO(#82): User-specified ignore patterns
        let ignored_patterns = vec!["@*", ".*"];

        let mut builder = GlobSetBuilder::new();

        for pattern in ignored_patterns {
            builder.add(Glob::new(pattern)?);
        }

        Ok(builder.build()?)
    }

    /// The root directory containing the working copy of sphere files on
    /// disk, as well as the local sphere data
    pub fn root_path(&self) -> &PathBuf {
        &self.root
    }

    /// The path to the sphere data folder within the working file tree
    pub fn sphere_path(&self) -> &PathBuf {
        &self.sphere
    }

    /// The path to the block storage database within the working file tree
    pub fn blocks_path(&self) -> &PathBuf {
        &self.blocks
    }

    /// The path to the folder that contains global Noosphere configuration
    /// and keys generated by the user
    pub fn noosphere_path(&self) -> &PathBuf {
        &self.noosphere
    }

    /// The path to the folder containing user-generated keys when there is
    /// no secure option for generating them available
    pub fn keys_path(&self) -> &PathBuf {
        &self.keys
    }

    /// Path to the local authorization (the granted UCAN) for the key that
    /// is authorized to work on the sphere
    pub fn authorization_path(&self) -> &PathBuf {
        &self.authorization
    }

    /// The path to the file containing the DID of the local key used to operate
    /// on the local sphere
    pub fn key_path(&self) -> &PathBuf {
        &self.key
    }

    /// The path to the file containing the DID of the sphere that is being
    /// worked on in this local workspace
    pub fn identity_path(&self) -> &PathBuf {
        &self.identity
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config
    }

    pub async fn get_local_gateway_url(&self) -> Result<Url> {
        match Config::from(self).read().await? {
            ConfigContents {
                gateway_url: Some(url),
                ..
            } => Ok(url.clone()),
            _ => Err(anyhow!(
                "No gateway URL configured; set it with: orb config set gateway-url <URL>"
            )),
        }
    }

    /// Attempts to read the locally stored authorization that enables the key
    /// to operate on this sphere; the returned authorization may be represented
    /// as either a UCAN or the CID of a UCAN
    pub async fn get_local_authorization(&self) -> Result<Authorization> {
        self.expect_local_directories()?;

        Authorization::from_str(&fs::read_to_string(&self.authorization).await?)
    }

    /// Produces a `SphereDb<NativeStore>` referring to the block storage
    /// backing the sphere in the local workspace
    pub async fn get_local_db(&self) -> Result<SphereDb<NativeStore>> {
        self.expect_local_directories()?;

        let storage_provider =
            NativeStorageProvider::new(NativeStorageInit::Path(self.blocks_path().clone()))?;
        SphereDb::new(&storage_provider).await
    }

    /// Get the key material (with both verification and signing capabilities)
    /// for the locally configured author key.
    pub async fn get_local_key(&self) -> Result<Ed25519KeyMaterial> {
        self.expect_global_directories()?;
        self.expect_local_directories()?;

        let local_key_did = fs::read_to_string(&self.key).await?;
        let keys = self.get_all_keys().await?;

        for (key, did) in keys {
            if did == local_key_did {
                let private_key_mnemonic = self.get_key_mnemonic(&key).await?;
                return restore_ed25519_key(&private_key_mnemonic);
            }
        }

        Err(anyhow!(
            "Could not resolve private key material for {:?}",
            local_key_did
        ))
    }

    /// Get the identity of the sphere being worked on in the local workspace as
    /// a DID string
    pub async fn get_local_identity(&self) -> Result<String> {
        self.expect_local_directories()?;

        Ok(fs::read_to_string(&self.identity).await?)
    }

    /// Look up the DID for the key by its name
    pub async fn get_key_did(&self, name: &str) -> Result<String> {
        Ok(fs::read_to_string(self.keys.join(name).with_extension("public")).await?)
    }

    /// Get a mnemonic corresponding to the private portion of a give key by name
    async fn get_key_mnemonic(&self, name: &str) -> Result<String> {
        Ok(fs::read_to_string(self.keys.join(name).with_extension("private")).await?)
    }

    /// Returns true if there are no files in the configured root path
    pub async fn is_root_empty(&self) -> Result<bool> {
        let mut directory = fs::read_dir(&self.root).await?;

        Ok(if let Some(_) = directory.next_entry().await? {
            false
        } else {
            true
        })
    }

    /// Reads all the available keys and returns a map of their names to their
    /// DIDs
    pub async fn get_all_keys(&self) -> Result<BTreeMap<String, String>> {
        self.expect_global_directories()?;

        let mut key_names = BTreeMap::<String, String>::new();
        let mut directory = fs::read_dir(&self.keys).await?;

        while let Some(entry) = directory.next_entry().await? {
            let key_path = entry.path();
            let key_name = key_path.file_stem().map(|stem| stem.to_str());
            let extension = key_path.extension().map(|extension| extension.to_str());

            match (key_name, extension) {
                (Some(Some(key_name)), Some(Some("public"))) => {
                    let did = self.get_key_did(key_name).await?;
                    key_names.insert(key_name.to_string(), did);
                }
                _ => continue,
            };
        }

        Ok(key_names)
    }

    /// If there is only one key to choose from, returns its name. Otherwise
    /// returns an error result.
    pub async fn unambiguous_default_key_name(&self) -> Result<String> {
        if self.expect_global_directories().is_ok() {
            let keys = self.get_all_keys().await?;

            if keys.len() > 1 {
                let key_names = keys
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<String>>()
                    .join("\n");
                return Err(anyhow!(
                    r#"There is more than one key; you should specify a key to use by name
The available keys are:

{}"#,
                    key_names
                ));
            } else if let Some((key_name, _)) = keys.iter().next() {
                return Ok(key_name.clone());
            }
        }

        Err(anyhow!("No keys found; have you created any yet?"))
    }

    /// Asserts that all related directories for the suggested working file
    /// tree root are present
    pub fn expect_local_directories(&self) -> Result<()> {
        if !self.root.is_dir() {
            return Err(anyhow!(
                "Configured sphere root {:?} is not a directory!",
                self.root
            ));
        }

        if !self.sphere.is_dir() {
            return Err(anyhow!(
                "The {:?} folder within {:?} is missing or corrupted",
                SPHERE_DIRECTORY,
                self.root
            ));
        }

        Ok(())
    }

    /// Asserts that the global Noosphere directories are present
    pub fn expect_global_directories(&self) -> Result<()> {
        if !self.noosphere.is_dir() || !self.keys.is_dir() {
            return Err(anyhow!(
                "The Noosphere config directory ({:?}) is missing or corrupted",
                self.noosphere
            ));
        }

        Ok(())
    }

    /// Creates all the directories needed to start rendering a sphere in the
    /// configured working file tree root
    pub async fn initialize_local_directories(&self) -> Result<()> {
        let mut root = self.root.clone();

        // Crawl up the directories to the root of the filesystem and make sure
        // we aren't initializing a sphere within a sphere
        while let Some(parent) = root.clone().parent() {
            root = parent.to_path_buf();
            let working_paths = Workspace::new(&root, None)?;
            if let Ok(_) = working_paths.expect_local_directories() {
                return Err(anyhow!(
                    r#"Tried to initialize sphere directories in {:?}
...but a sphere is already initialized in {:?}
Unexpected things will happen if you try to nest spheres this way!"#,
                    self.root,
                    parent
                ))?;
            }
        }

        fs::create_dir_all(&self.sphere).await?;

        fs::write(self.config_path(), "").await?;

        Ok(())
    }

    /// Creates the global Noosphere config and keys directories
    pub async fn initialize_global_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.keys).await?;

        Ok(())
    }

    pub fn new(root: &PathBuf, noosphere_global_root: Option<&PathBuf>) -> Result<Self> {
        if !root.is_absolute() {
            return Err(anyhow!("Ambiguous path to sphere root: {:?}", root));
        }

        let root = root.absolutize()?.to_path_buf();
        let sphere = root.join(SPHERE_DIRECTORY);
        let blocks = sphere.join(BLOCKS_DIRECTORY);
        let authorization = sphere.join(AUTHORIZATION_FILE);
        let key = sphere.join(KEY_FILE);
        let identity = sphere.join(IDENTITY_FILE);
        let config = sphere.join(CONFIG_FILE);
        let noosphere = match noosphere_global_root {
            Some(custom_root) => custom_root.clone(),
            None => home::home_dir()
                .ok_or_else(|| {
                    anyhow!(
                        "Could not discover home directory for {}",
                        whoami::username()
                    )
                })?
                .join(NOOSPHERE_DIRECTORY),
        };
        let keys = noosphere.join(KEYS_DIRECTORY);

        Ok(Workspace {
            root,
            sphere,
            blocks,
            authorization,
            key,
            identity,
            noosphere,
            keys,
            config,
        })
    }

    #[cfg(test)]
    pub fn temporary() -> Result<Self> {
        use temp_dir::TempDir;

        let root = TempDir::new()?;
        let global_root = TempDir::new()?;

        Workspace::new(
            &root.path().to_path_buf(),
            Some(&global_root.path().to_path_buf()),
        )
    }
}
