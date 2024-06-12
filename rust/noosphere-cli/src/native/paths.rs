//! Implementation related to the file system layout of a sphere workspace

use anyhow::{anyhow, Result};
use cid::{multihash::Code, multihash::MultihashDigest, Cid};
use libipld_core::raw::RawCodec;
use noosphere_core::data::{Did, MemoIpld};
use noosphere_storage::base64_encode;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use super::extension::infer_file_extension;

/// The name of the "private" sphere folder, similar to a .git folder, that is
/// used to record and update the structure of a sphere over time
pub const SPHERE_DIRECTORY: &str = ".sphere";

pub(crate) const STORAGE_DIRECTORY: &str = "storage";
pub(crate) const CONTENT_DIRECTORY: &str = "content";
pub(crate) const PEERS_DIRECTORY: &str = "peers";
pub(crate) const SLUGS_DIRECTORY: &str = "slugs";
pub(crate) const MOUNT_DIRECTORY: &str = "mount";
pub(crate) const VERSION_FILE: &str = "version";
pub(crate) const IDENTITY_FILE: &str = "identity";
pub(crate) const DEPTH_FILE: &str = "depth";
pub(crate) const LINK_RECORD_FILE: &str = "link_record";

/// [SpherePaths] record the critical paths within a sphere workspace as
/// rendered to a typical file system. It is used to ensure that we read from
/// and write to consistent locations when rendering and updating a sphere as
/// files on disk.
///
/// NOTE: We use hashes to represent internal paths for a couple of reasons,
/// both related to Windows filesystem limitations:
///
///  1. Windows filesystem, in the worst case, only allows 260 character-long
///     paths
///  2. Windows does not allow various characters (e.g., ':') in file paths, and
///     there is no option to escape those characters
///
/// Hashing eliminates problem 2 and improves conditions so that we are more
/// likely to avoid problem 1.
///
/// See:
/// https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation?tabs=registry
/// See also:
/// https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#naming-conventions
#[derive(Clone)]
pub struct SpherePaths {
    root: PathBuf,
    sphere: PathBuf,
    storage: PathBuf,
    slugs: PathBuf,
    content: PathBuf,
    peers: PathBuf,
    version: PathBuf,
    identity: PathBuf,
    depth: PathBuf,
}

impl Debug for SpherePaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpherePaths")
            .field("root", &self.root)
            .finish()
    }
}

impl SpherePaths {
    /// Returns true if the given path has a .sphere folder
    fn has_sphere_directory(path: &Path) -> bool {
        path.is_absolute() && path.join(SPHERE_DIRECTORY).is_dir()
    }

    /// Construct a new [SpherePaths] given a `root` path, a directory
    /// that will contain a `.sphere` directory.
    pub fn new(root: &Path) -> Self {
        let sphere = root.join(SPHERE_DIRECTORY);

        Self {
            root: root.into(),
            storage: sphere.join(STORAGE_DIRECTORY),
            content: sphere.join(CONTENT_DIRECTORY),
            peers: sphere.join(PEERS_DIRECTORY),
            slugs: sphere.join(SLUGS_DIRECTORY),
            version: sphere.join(VERSION_FILE),
            identity: sphere.join(IDENTITY_FILE),
            depth: sphere.join(DEPTH_FILE),
            sphere,
        }
    }

    /// Initialize [SpherePaths] given its root path. This has the effect of
    /// creating the "private" directory hierarchy (starting from
    /// [SPHERE_DIRECTORY] inside the root).
    pub async fn initialize(&self) -> Result<()> {
        if !self.root.is_absolute() {
            return Err(anyhow!(
                "Must use an absolute path to initialize sphere directories; got {:?}",
                self.root
            ));
        }

        std::fs::create_dir_all(&self.storage)?;
        std::fs::create_dir_all(&self.content)?;
        std::fs::create_dir_all(&self.peers)?;
        std::fs::create_dir_all(&self.slugs)?;

        Ok(())
    }

    /// Attempt to discover an existing workspace root by traversing ancestor
    /// directories until one is found that contains a [SPHERE_DIRECTORY].
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

    /// The path to the root version file within the local [SPHERE_DIRECTORY]
    pub fn version(&self) -> &Path {
        &self.version
    }

    /// The path to the root identity file within the local [SPHERE_DIRECTORY]
    pub fn identity(&self) -> &Path {
        &self.identity
    }

    /// The path to the root depth file within the local [SPHERE_DIRECTORY]
    pub fn depth(&self) -> &Path {
        &self.depth
    }

    /// The path to the workspace root directory, which contains a
    /// [SPHERE_DIRECTORY]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path to the [SPHERE_DIRECTORY] within the workspace root
    pub fn sphere(&self) -> &Path {
        &self.sphere
    }

    /// The path the directory within the [SPHERE_DIRECTORY] that contains
    /// rendered peer spheres
    pub fn peers(&self) -> &Path {
        &self.peers
    }

    /// Given a slug, get a path where we may write a reverse-symlink to a file
    /// system file that is a rendered equivalent of the content that can be
    /// found at that slug. The slug's UTF-8 bytes are base64-encoded so that
    /// certain characters that are allowed in slugs (e.g., '/') do not prevent
    /// us from creating the symlink.
    pub fn slug(&self, slug: &str) -> Result<PathBuf> {
        Ok(self.slugs.join(base64_encode(slug.as_bytes())?))
    }

    /// Given a peer [Did] and sphere version [Cid], get a path where the that
    /// peer's sphere at the given version ought to be rendered. The path will
    /// be unique and deterministic far a given combination of [Did] and [Cid].
    pub fn peer(&self, peer: &Did, version: &Cid) -> PathBuf {
        let cid = Cid::new_v1(
            RawCodec.into(),
            Code::Blake3_256.digest(&[peer.as_bytes(), &version.to_bytes()].concat()),
        );
        self.peers.join(cid.to_string())
    }

    /// Given a [Cid] for a peer's memo, get a path to a file where the content
    /// referred to by that memo ought to be written.
    pub fn peer_hard_link(&self, memo_cid: &Cid) -> PathBuf {
        self.content.join(memo_cid.to_string())
    }

    /// Given a slug and a [MemoIpld] referring to some content in the local
    /// sphere, get a path to a file where the content referred to by the
    /// [MemoIpld] ought to be rendered (including file extension).
    pub fn root_hard_link(&self, slug: &str, memo: &MemoIpld) -> Result<PathBuf> {
        self.file(&self.root, slug, memo)
    }

    /// Similar to [SpherePaths::root_hard_link] but for a peer given by [Did]
    /// and sphere version [Cid].
    pub fn peer_soft_link(
        &self,
        peer: &Did,
        version: &Cid,
        slug: &str,
        memo: &MemoIpld,
    ) -> Result<PathBuf> {
        self.file(&self.peer(peer, version).join(MOUNT_DIRECTORY), slug, memo)
    }

    /// Given a base path, a slug and a [MemoIpld], get the full file path
    /// (including inferred file extension) for a file that corresponds to the
    /// given [MemoIpld].
    pub fn file(&self, base: &Path, slug: &str, memo: &MemoIpld) -> Result<PathBuf> {
        let extension = infer_file_extension(memo)?;
        let file_fragment = match extension {
            Some(extension) => [slug, &extension].join("."),
            None => slug.into(),
        };
        Ok(base.join(file_fragment))
    }
}
