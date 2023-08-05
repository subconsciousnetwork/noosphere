//! Operations that are common to most CLI commands

use anyhow::{anyhow, Result};
use cid::Cid;
use directories::ProjectDirs;
use noosphere::sphere::SphereContextBuilder;
use noosphere_core::authority::Author;
use noosphere_core::data::{Did, Link, LinkRecord, MemoIpld};
use noosphere_sphere::{SphereContentRead, SphereContext, SphereCursor, COUNTERPART, GATEWAY_URL};
use noosphere_storage::{KeyValueStore, NativeStorage, SphereDb};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use ucan::crypto::KeyMaterial;
use url::Url;

use noosphere::key::InsecureKeyStorage;
use tokio::sync::{Mutex, OnceCell};

use crate::native::paths::{IDENTITY_FILE, LINK_RECORD_FILE, VERSION_FILE};

use super::paths::SpherePaths;
use super::render::SphereRenderer;

/// The flavor of [SphereContext] used through the CLI
pub type CliSphereContext = SphereContext<NativeStorage>;

/// Metadata about a given sphere, including the sphere ID, a [Link]
/// to it and a corresponding [LinkRecord] (if one is available).
pub type SphereDetails = (Did, Link<MemoIpld>, Option<LinkRecord>);

/// The [Workspace] is the kernel of the CLI. It implements it keeps state and
/// implements routines that are common to most CLI commands.
pub struct Workspace {
    sphere_paths: Option<Arc<SpherePaths>>,
    key_storage: InsecureKeyStorage,
    sphere_context: OnceCell<Arc<Mutex<CliSphereContext>>>,
    working_directory: PathBuf,
}

impl Workspace {
    /// The current working directory as given to the [Workspace] when it was
    /// created
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    /// Get a mutex-guarded reference to the [SphereContext] for the current workspace
    pub async fn sphere_context(&self) -> Result<Arc<Mutex<CliSphereContext>>> {
        Ok(self
            .sphere_context
            .get_or_try_init(|| async {
                Ok(Arc::new(Mutex::new(
                    SphereContextBuilder::default()
                        .open_sphere(None)
                        .at_storage_path(self.require_sphere_paths()?.root())
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

    /// Get the [Author] that is configured to work on the local sphere
    pub async fn author(&self) -> Result<Author<impl KeyMaterial + Clone>> {
        Ok(self.sphere_context().await?.lock().await.author().clone())
    }

    /// Same as [Workspace::sphere_paths] but returns an error result if the
    /// [SpherePaths] have not been initialized for this [Workspace].
    pub fn require_sphere_paths(&self) -> Result<&Arc<SpherePaths>> {
        self.sphere_paths
            .as_ref()
            .ok_or_else(|| anyhow!("Sphere paths not discovered for this location"))
    }

    /// Get the [SpherePaths] for this workspace, if they have been initialized
    /// and/or discovered.
    pub fn sphere_paths(&self) -> Option<&Arc<SpherePaths>> {
        self.sphere_paths.as_ref()
    }

    /// Gets the [Did] of the sphere
    pub async fn sphere_identity(&self) -> Result<Did> {
        let context = self.sphere_context().await?;
        let context = context.lock().await;

        Ok(context.identity().clone())
    }

    /// Get the configured counterpart sphere's identity (for a gateway, this is
    /// the client sphere ID; for a client, this is the gateway's sphere ID)
    pub async fn counterpart_identity(&self) -> Result<Did> {
        self.db().await?.require_key(COUNTERPART).await
    }

    /// Get the configured gateway URL for the local workspace
    pub async fn gateway_url(&self) -> Result<Url> {
        self.db().await?.require_key(GATEWAY_URL).await
    }

    /// Returns true if the local sphere has been initialized
    pub fn is_sphere_initialized(&self) -> bool {
        if let Some(sphere_paths) = self.sphere_paths() {
            sphere_paths.sphere().exists()
        } else {
            false
        }
    }

    /// Asserts that a local sphere has been intiialized
    pub fn ensure_sphere_initialized(&self) -> Result<()> {
        let sphere_paths = self.require_sphere_paths()?;
        if !sphere_paths.sphere().exists() {
            return Err(anyhow!(
                "Expected {} to exist!",
                sphere_paths.sphere().display()
            ));
        }
        Ok(())
    }

    /// Asserts that a local sphere has _not_ been intiialized
    pub fn ensure_sphere_uninitialized(&self) -> Result<()> {
        if let Some(sphere_paths) = self.sphere_paths() {
            match sphere_paths.sphere().exists() {
                true => {
                    return Err(anyhow!(
                        "A sphere is already initialized in {}",
                        sphere_paths.root().display()
                    ))
                }
                false => (),
            }
        }

        Ok(())
    }

    /// For a given location on disk, describe the closest sphere by traversing
    /// file system ancestors until a sphere is found (either the root workspace
    /// or one of the rendered peers within that workspace).
    #[instrument(level = "trace", skip(self))]
    pub async fn describe_closest_sphere(
        &self,
        starting_from: Option<&Path>,
    ) -> Result<Option<SphereDetails>> {
        trace!("Looking for closest sphere...");

        let sphere_paths = self.require_sphere_paths()?;

        let canonical =
            tokio::fs::canonicalize(starting_from.unwrap_or_else(|| self.working_directory()))
                .await?;

        let peers = sphere_paths.peers();
        let root = sphere_paths.root();

        let mut sphere_base: &Path = &canonical;

        while let Some(parent) = sphere_base.parent() {
            trace!("Looking in {}...", parent.display());

            if parent == peers || parent == root {
                trace!("Found!");

                let (identity, version, link_record) = tokio::join!(
                    tokio::fs::read_to_string(sphere_base.join(IDENTITY_FILE)),
                    tokio::fs::read_to_string(sphere_base.join(VERSION_FILE)),
                    tokio::fs::read_to_string(sphere_base.join(LINK_RECORD_FILE)),
                );
                let identity = identity?;
                let version = version?;
                let link_record = if let Ok(link_record) = link_record {
                    LinkRecord::try_from(link_record).ok()
                } else {
                    None
                };

                return Ok(Some((
                    identity.into(),
                    Cid::try_from(version)?.into(),
                    link_record,
                )));
            } else {
                sphere_base = parent;
            }
        }

        Ok(None)
    }

    /// Reads a nickname from a blessed slug `_profile_`, which is used by
    /// Subconscious (the first embedder of Noosphere) to store user profile
    /// data as JSON.
    #[instrument(level = "trace", skip(self))]
    pub async fn read_subconscious_flavor_profile_nickname(
        &self,
        identity: &Did,
        version: &Link<MemoIpld>,
    ) -> Result<Option<String>> {
        trace!("Looking for profile nickname");
        let sphere_context = self.sphere_context().await?;
        let peer_sphere_context = Arc::new(sphere_context.lock().await.to_visitor(identity).await?);
        let cursor = SphereCursor::mounted_at(peer_sphere_context, version);

        if let Some(mut profile) = cursor.read("_profile_").await? {
            let mut profile_json = String::new();
            profile.contents.read_to_string(&mut profile_json).await?;
            match serde_json::from_str(&profile_json)? {
                Value::Object(object) => match object.get("nickname") {
                    Some(Value::String(nickname)) => Ok(Some(nickname.to_owned())),
                    _ => Ok(None),
                },
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Given a path, look for a petname within the path by traversing ancestors until a
    /// path component that starts with '@' is found.
    #[instrument(level = "trace", skip(self))]
    fn find_petname_in_path(&self, path: &Path) -> Result<Option<(String, PathBuf)>> {
        let mut current_path: Option<&Path> = Some(path);

        debug!("Looking for the petname of the local sphere...");
        while let Some(path) = current_path {
            trace!("Looking for petname in {}", path.display());
            if let Some(tail) = path.components().last() {
                if let Some(str) = tail.as_os_str().to_str() {
                    if str.starts_with('@') {
                        let petname = str.split('@').last().unwrap_or_default().to_owned();
                        debug!("Found petname @{}", petname);
                        return Ok(Some((petname, path.to_owned())));
                    }
                }
            }

            current_path = path.parent();
        }

        debug!("No petname found");
        Ok(None)
    }

    /// Reads the latest local version of the sphere and renders its contents to
    /// files in the workspace. Note that this will overwrite any existing files
    /// in the workspace.
    #[instrument(level = "debug", skip(self))]
    pub async fn render(&self, depth: Option<u32>, force_full: bool) -> Result<()> {
        let renderer = SphereRenderer::new(
            self.sphere_context().await?,
            self.require_sphere_paths()?.clone(),
        );

        renderer.render(depth, force_full).await?;

        Ok(())
    }

    /// Initialize a [Workspace] in place with a given set of [SpherePaths].
    pub fn initialize(&mut self, sphere_paths: SpherePaths) -> Result<()> {
        self.ensure_sphere_uninitialized()?;

        self.sphere_paths = Some(Arc::new(sphere_paths));

        Ok(())
    }

    /// Create a new (possibly uninitialized) [Workspace] for a given working
    /// directory and optional global configuration directory.
    ///
    /// This constructor will attempt to discover the [SpherePaths] by traversing
    /// ancestors from the provided working directory. The [Workspace] will be considered
    /// initialized if [SpherePaths] are discovered, otherwise it will be considered
    /// uninitialized.
    ///
    /// If no global configuration directory is specified, one will be automatically
    /// chosen based on the current platform:
    ///
    /// - Linux: /home/<user>/.config/noosphere
    /// - MacOS: /Users/<user>/Library/Application Support/network.subconscious.noosphere
    /// - Windows: C:\Users\<user>\AppData\Roaming\subconscious\noosphere\config
    ///
    /// On Linux, an $XDG_CONFIG_HOME environment variable will be respected if set.
    pub fn new(
        working_directory: &Path,
        custom_noosphere_directory: Option<&Path>,
    ) -> Result<Self> {
        let sphere_paths = SpherePaths::discover(Some(working_directory)).map(Arc::new);

        let noosphere_directory = match custom_noosphere_directory {
            Some(path) => path.to_owned(),
            None => {
                // NOTE: Breaking change for key storage location here
                let project_dirs = ProjectDirs::from("network", "subconscious", "noosphere")
                    .ok_or_else(|| anyhow!("Unable to determine noosphere config directory"))?;
                project_dirs.config_dir().to_owned()
            }
        };

        debug!(
            "Initializing key storage from {}",
            noosphere_directory.display()
        );

        let key_storage = InsecureKeyStorage::new(&noosphere_directory)?;

        let workspace = Workspace {
            sphere_paths,
            key_storage,
            sphere_context: OnceCell::new(),
            working_directory: working_directory.to_owned(),
        };

        Ok(workspace)
    }
}
