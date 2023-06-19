use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::data::{Did, MemoIpld};
use noosphere_storage::base64_encode;
use std::path::{Path, PathBuf};

use super::extension::infer_file_extension;

pub const SPHERE_DIRECTORY: &str = ".sphere";
pub const NOOSPHERE_DIRECTORY: &str = ".noosphere";
pub const STORAGE_DIRECTORY: &str = "storage";
pub const CONTENT_DIRECTORY: &str = "content";
pub const PEERS_DIRECTORY: &str = "peers";
pub const SLUGS_DIRECTORY: &str = "slugs";
pub const VERSION_FILE: &str = "version";

#[derive(Debug, Clone)]
pub struct SpherePaths {
    root: PathBuf,
    sphere: PathBuf,
    storage: PathBuf,
    slugs: PathBuf,
    content: PathBuf,
    peers: PathBuf,
    version: PathBuf,
}

impl SpherePaths {
    /// Returns true if the given path has a .sphere folder
    fn has_sphere_directory(path: &Path) -> bool {
        path.is_absolute() && path.join(SPHERE_DIRECTORY).is_dir()
    }

    // Root is the path that contains the .sphere folder
    fn new(root: &Path) -> Self {
        let sphere = root.join(SPHERE_DIRECTORY);

        Self {
            root: root.into(),
            storage: sphere.join(STORAGE_DIRECTORY),
            content: sphere.join(CONTENT_DIRECTORY),
            peers: sphere.join(PEERS_DIRECTORY),
            slugs: sphere.join(SLUGS_DIRECTORY),
            version: sphere.join(VERSION_FILE),
            sphere,
        }
    }

    pub async fn intialize(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err(anyhow!(
                "Must use an absolute path to initialize sphere directories; got {:?}",
                root
            ));
        }

        let paths = Self::new(root);

        std::fs::create_dir_all(&paths.storage)?;
        std::fs::create_dir_all(&paths.content)?;
        std::fs::create_dir_all(&paths.peers)?;
        std::fs::create_dir_all(&paths.slugs)?;

        Ok(paths)
    }

    #[instrument(level = "trace")]
    pub fn discover(from: Option<&Path>) -> Option<Self> {
        trace!("Looking in {:?}", from);

        match from {
            Some(directory) => {
                if Self::has_sphere_directory(directory) {
                    trace!("Found in {:?}!", directory);
                    Some(Self::new(directory))
                } else {
                    Self::discover(directory.parent())
                }
            }
            None => None,
        }
    }

    pub fn version(&self) -> &Path {
        &self.version
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sphere(&self) -> &Path {
        &self.sphere
    }

    pub fn slug(&self, slug: &str) -> Result<PathBuf> {
        Ok(self.slugs.join(base64_encode(slug.as_bytes())?))
    }

    pub fn peer(&self, peer: &Did, version: &Cid) -> PathBuf {
        self.peers.join(peer.as_str()).join(&version.to_string())
    }

    pub fn root_content(&self, slug: &str, memo: &MemoIpld) -> Result<PathBuf> {
        let extension = infer_file_extension(memo)?;
        let file_fragment = match extension {
            Some(extension) => [slug, &extension].join("."),
            None => slug.into(),
        };
        Ok(self.root.join(file_fragment))
    }

    pub fn peer_content(&self, memo_cid: &Cid) -> PathBuf {
        self.content.join(&memo_cid.to_string())
    }
}
