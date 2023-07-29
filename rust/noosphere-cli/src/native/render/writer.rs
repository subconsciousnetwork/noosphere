use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::data::Did;
use noosphere_sphere::{AsyncFileBody, SphereFile};
use pathdiff::diff_paths;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use symlink::{remove_symlink_dir, remove_symlink_file, symlink_dir, symlink_file};
use tokio::{fs::File, io::copy};

use crate::native::paths::{SpherePaths, MOUNT_DIRECTORY};

use super::JobKind;

#[derive(Debug, Clone)]
pub struct SphereWriter {
    kind: JobKind,
    paths: Arc<SpherePaths>,
    base: OnceLock<PathBuf>,
    mount: OnceLock<PathBuf>,
    private: OnceLock<PathBuf>,
}

impl SphereWriter {
    pub fn new(kind: JobKind, paths: Arc<SpherePaths>) -> Self {
        SphereWriter {
            kind,
            paths,
            base: Default::default(),
            mount: Default::default(),
            private: Default::default(),
        }
    }

    fn is_root_writer(&self) -> bool {
        self.kind == JobKind::Root
    }

    fn petname(&self, name: &str) -> PathBuf {
        self.mount().join(format!("@{}", name))
    }

    pub fn mount(&self) -> &Path {
        self.mount.get_or_init(|| match &self.kind {
            JobKind::Root => self.base().to_owned(),
            JobKind::Peer(_, _) => self.base().join(MOUNT_DIRECTORY),
        })
    }

    pub fn base(&self) -> &Path {
        self.base.get_or_init(|| match &self.kind {
            JobKind::Root => self.paths.root().to_owned(),
            JobKind::Peer(did, cid) => self.paths.peer(did, cid),
        })
    }

    pub fn private(&self) -> &Path {
        self.private.get_or_init(|| match &self.kind {
            JobKind::Root => self.paths().sphere().to_owned(),
            JobKind::Peer(_, _) => self.base().to_owned(),
        })
    }

    pub fn paths(&self) -> &SpherePaths {
        &self.paths
    }

    pub async fn write_identity_and_version(&self, identifier: &Did, version: &Cid) -> Result<()> {
        let private = self.private();

        let (id_result, version_result) = tokio::join!(
            tokio::fs::write(private.join("identifier"), identifier.to_string()),
            tokio::fs::write(private.join("version"), version.to_string())
        );

        id_result?;
        version_result?;

        Ok(())
    }

    /// Resolves the path to the hard link-equivalent file that contains the
    /// content for this slug. A [SphereFile] is required because we need a
    /// [MemoIpld] in the case of rendering root, and we need the [Cid] of that
    /// [MemoIpld] when rendering a peer. Both are conveniently bundled by
    /// a [SphereFile].
    pub fn content_hard_link<R>(&self, slug: &str, file: &SphereFile<R>) -> Result<PathBuf> {
        if self.is_root_writer() {
            self.paths.root_hard_link(slug, &file.memo)
        } else {
            Ok(self.paths.peer_hard_link(&file.memo_version))
        }
    }

    #[instrument]
    pub async fn remove_content(&self, slug: &str) -> Result<()> {
        if self.is_root_writer() {
            let slug_path = self.paths.slug(slug)?;
            let file_path = tokio::fs::read_link(&slug_path).await?;

            let _ = remove_symlink_file(slug_path);

            if file_path.exists() {
                trace!("Removing '{}'", file_path.display());
                tokio::fs::remove_file(&file_path).await?;
            }

            Ok(())
        } else {
            Err(anyhow!("Cannot 'remove' individual peer content"))
        }
    }

    #[instrument(skip(file))]
    pub async fn write_content<R>(&self, slug: &str, file: &mut SphereFile<R>) -> Result<()>
    where
        R: AsyncFileBody,
    {
        let file_path = self.content_hard_link(slug, file)?;

        trace!("Final file path will be '{}'", file_path.display());

        let file_directory = file_path
            .parent()
            .ok_or_else(|| anyhow!("Unable to determine base directory for '{}'", slug))?;

        tokio::fs::create_dir_all(file_directory).await?;

        match tokio::fs::try_exists(&file_path).await {
            Ok(true) => {
                trace!("'{}' content already exists, not re-rendering...", slug);
            }
            Err(error) => {
                warn!("Error checking for existing file: {}", error);
            }
            _ => {
                debug!("Rendering content for '{}'...", slug);
                let mut fs_file = File::create(&file_path).await?;
                copy(&mut file.contents, &mut fs_file).await?;
            }
        };

        // If we are writing root, we need to symlink from inside .sphere to the
        // rendered file (we use this backlink to determine how moves / deletes
        // should be recorded when saving); if we are writing to a peer, we need
        // to symlink from the end-user visible filesystem location into the
        // content location within .sphere (so the links go the other
        // direction).
        if self.is_root_writer() {
            self.symlink_slug(slug, &file_path).await?;
        } else {
            self.symlink_content(
                &file.memo_version,
                &self.paths.file(self.mount(), slug, &file.memo)?,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn symlink_content(&self, memo_cid: &Cid, file_path: &PathBuf) -> Result<()> {
        let file_directory_path = file_path.parent().ok_or_else(|| {
            anyhow!(
                "Unable to determine base directory for '{}'",
                file_path.display()
            )
        })?;

        tokio::fs::create_dir_all(file_directory_path).await?;

        let relative_peer_content_path =
            diff_paths(self.paths.peer_hard_link(memo_cid), file_directory_path).ok_or_else(
                || {
                    anyhow!(
                        "Could not resolve relative path for '{}'",
                        file_path.display()
                    )
                },
            )?;

        trace!(
            "Symlinking from '{}' to '{}'...",
            relative_peer_content_path.display(),
            file_path.display()
        );

        if file_path.exists() {
            remove_symlink_file(file_path)?;
        }

        symlink_file(relative_peer_content_path, file_path)?;

        Ok(())
    }

    pub async fn unlink_peer(&self, petname: &str) -> Result<()> {
        let absolute_peer_destination = self.petname(petname);
        if absolute_peer_destination.exists() {
            remove_symlink_dir(absolute_peer_destination)?;
        }
        Ok(())
    }

    pub async fn symlink_peer(&self, peer: &Did, version: &Cid, petname: &str) -> Result<()> {
        let absolute_peer_destination = self.petname(petname);
        let peer_directory_path = absolute_peer_destination.parent().ok_or_else(|| {
            anyhow!(
                "Unable to determine base directory for '{}'",
                absolute_peer_destination.display()
            )
        })?;

        tokio::fs::create_dir_all(peer_directory_path).await?;

        let relative_peer_source = diff_paths(
            self.paths.peer(peer, version).join(MOUNT_DIRECTORY),
            self.mount(),
        )
        .ok_or_else(|| anyhow!("Could not resolve relative path for to '@{petname}'",))?;

        self.unlink_peer(petname).await?;

        symlink_dir(relative_peer_source, absolute_peer_destination)?;

        Ok(())
    }

    pub async fn symlink_slug(&self, slug: &str, file_path: &PathBuf) -> Result<()> {
        let slug_path = self.paths.slug(slug)?;
        let slug_base = slug_path
            .parent()
            .ok_or_else(|| anyhow!("Can't resolve parent directory of {}", slug_path.display()))?;

        let relative_file_path = diff_paths(file_path, slug_base).ok_or_else(|| {
            anyhow!(
                "Could not resolve relative path for '{}'",
                file_path.display()
            )
        })?;

        if slug_path.exists() {
            remove_symlink_file(&slug_path)?;
        }

        symlink_file(relative_file_path, slug_path)?;

        Ok(())
    }
}
