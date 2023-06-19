use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::data::{Did, MemoIpld};
use noosphere_sphere::{AsyncFileBody, SphereFile};
use std::path::{Path, PathBuf};
use symlink::{symlink_dir, symlink_file};
use tokio::{fs::File, io::copy};

use super::extension::infer_file_extension;

const SPHERE_DIRECTORY: &str = ".sphere";
const NOOSPHERE_DIRECTORY: &str = ".noosphere";
const STORAGE_DIRECTORY: &str = "storage";
const CONTENT_DIRECTORY: &str = "content";
const PEERS_DIRECTORY: &str = "peers";
const VERSION_FILE: &str = "version";

#[derive(Debug)]
pub struct SphereWriter<'a> {
    paths: &'a SpherePaths,
    base: PathBuf,
}

impl<'a> SphereWriter<'a> {
    pub fn new(paths: &'a SpherePaths) -> Self {
        SphereWriter {
            paths,
            base: paths.root().to_path_buf(),
        }
    }

    pub fn is_root_writer(&self) -> bool {
        &self.base == self.paths.root()
    }

    pub fn descend(&self, peer: &Did, version: &Cid) -> Self {
        SphereWriter {
            paths: self.paths,
            base: self.paths.peer(peer, version),
        }
    }

    pub fn content_path<R>(&self, slug: &str, file: &SphereFile<R>) -> Result<PathBuf> {
        if self.is_root_writer() {
            self.paths.root_content(slug, &file.memo)
        } else {
            Ok(self.paths.peer_content(&file.memo_version))
        }
    }

    #[instrument(skip(file))]
    pub async fn write_content<R>(&self, slug: &str, file: &mut SphereFile<R>) -> Result<()>
    where
        R: AsyncFileBody,
    {
        let file_path = self.content_path(slug, file)?;
        let file_directory = file_path
            .parent()
            .ok_or_else(|| anyhow!("Unable to determine base directory for '{}'", slug))?;

        tokio::fs::create_dir_all(file_directory).await?;

        match tokio::fs::try_exists(&file_path).await {
            Ok(true) => {
                trace!("'{}' content already exists, skipping...", slug);
                return Ok(());
            }
            Err(error) => {
                warn!("Error checking for existing file: {}", error);
                ()
            }
            _ => (),
        };

        let mut fs_file = File::create(file_path).await?;

        copy(&mut file.contents, &mut fs_file).await?;

        if !self.is_root_writer() {
            self.symlink_content(memo_cid, file_name);
        }

        Ok(())
    }

    pub fn symlink_content(&self, memo_cid: &Cid, file_name: &str) -> Result<()> {
        symlink_file(self.paths.peer_content(memo_cid), self.base.join(file_name))?;

        Ok(())
    }

    pub fn symlink_peer(&self, peer: &Did, version: &Cid, petname: &str) -> Result<()> {
        symlink_dir(
            self.paths.peer(peer, version),
            self.base.join(&format!("@{}", petname)),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SpherePaths {
    root: PathBuf,
    sphere: PathBuf,
    storage: PathBuf,
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
