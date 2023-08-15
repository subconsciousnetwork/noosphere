//! Helpers for working with the file system content within a workspace

use anyhow::{anyhow, Result};
use cid::Cid;
use globset::{Glob, GlobSet, GlobSetBuilder};
use noosphere_core::data::{BodyChunkIpld, ContentType};
use noosphere_storage::{BlockStore, MemoryStore};
use pathdiff::diff_paths;
use std::collections::{BTreeMap, BTreeSet};
use subtext::util::to_slug;
use tokio::fs;
use tokio_stream::StreamExt;

use noosphere_sphere::SphereWalker;

use super::{extension::infer_content_type, paths::SpherePaths, workspace::Workspace};

/// Metadata that identifies some sphere content that is present on the file
/// system
pub struct FileReference {
    /// The [Cid] of the file's body contents
    pub cid: Cid,
    /// The inferred [ContentType] of the file
    pub content_type: ContentType,
    /// The known extension of the file, if any
    pub extension: Option<String>,
}

/// A delta manifest of changes to the local content space
#[derive(Default)]
pub struct ContentChanges {
    /// Newly added files
    pub new: BTreeMap<String, Option<ContentType>>,
    /// Updated files
    pub updated: BTreeMap<String, Option<ContentType>>,
    /// Removed files
    pub removed: BTreeMap<String, Option<ContentType>>,
    /// Unchanged files
    pub unchanged: BTreeSet<String>,
}

impl ContentChanges {
    /// Returns true if there are no recorded changes
    pub fn is_empty(&self) -> bool {
        self.new.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

/// A manifest of content to apply some work to in the local content space
#[derive(Default)]
pub struct Content {
    /// Content in the workspace that can be considered for inclusion in the
    /// sphere's content space
    pub matched: BTreeMap<String, FileReference>,
    /// Content in the workspace that has been ignored
    pub ignored: BTreeSet<String>,
}

impl Content {
    /// Returns true if no content has been found that can be included in the
    /// sphere's content space
    pub fn is_empty(&self) -> bool {
        self.matched.is_empty()
    }

    /// Produce a matcher that will match any path that should be ignored when
    /// considering the files that make up the local workspace
    fn get_ignored_patterns() -> Result<GlobSet> {
        // TODO(#82): User-specified ignore patterns
        let ignored_patterns = vec!["@*", ".*"];

        let mut builder = GlobSetBuilder::new();

        for pattern in ignored_patterns {
            builder.add(Glob::new(pattern)?);
        }

        Ok(builder.build()?)
    }

    /// Read the local content of the workspace in its entirety.
    /// This includes files that have not yet been saved to the sphere. All
    /// files are chunked into blocks, and those blocks are persisted to the
    /// provided store.
    // TODO(#556): This is slow; we could probably do a concurrent traversal
    // similar to how we traverse when rendering files to disk
    pub async fn read_all<S: BlockStore>(paths: &SpherePaths, store: &mut S) -> Result<Content> {
        let root_path = paths.root();
        let mut directories = vec![(None, tokio::fs::read_dir(root_path).await?)];

        let ignore_patterns = Content::get_ignored_patterns()?;
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

                    // TODO(#557): Limit the depth of the directory traversal to
                    // some reasonable number

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
                    Some(extension) => infer_content_type(extension)?,
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

    /// Read all changed content in the sphere's workspace. Changed content will
    /// include anything that has been modified, moved or deleted. The blocks
    /// associated with the changed content will be included in the returned
    /// [MemoryStore].
    pub async fn read_changes(
        workspace: &Workspace,
    ) -> Result<Option<(Content, ContentChanges, MemoryStore)>> {
        // TODO(#556): We need a better strategy than reading all changed
        // content into memory at once
        let mut new_blocks = MemoryStore::default();
        let file_content =
            Content::read_all(workspace.require_sphere_paths()?, &mut new_blocks).await?;

        let sphere_context = workspace.sphere_context().await?;
        let walker = SphereWalker::from(&sphere_context);

        let content_stream = walker.content_stream();
        tokio::pin!(content_stream);

        let mut changes = ContentChanges::default();

        while let Some((slug, sphere_file)) = content_stream.try_next().await? {
            if file_content.ignored.contains(&slug) {
                continue;
            }

            match file_content.matched.get(&slug) {
                Some(FileReference {
                    cid: body_cid,
                    content_type,
                    extension: _,
                }) => {
                    if &sphere_file.memo.body == body_cid {
                        changes.unchanged.insert(slug.clone());
                        continue;
                    }

                    changes
                        .updated
                        .insert(slug.clone(), Some(content_type.clone()));
                }
                None => {
                    changes
                        .removed
                        .insert(slug.clone(), sphere_file.memo.content_type());
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

        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some((file_content, changes, new_blocks)))
        }
    }
}
