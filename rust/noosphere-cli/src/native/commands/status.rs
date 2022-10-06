use std::collections::BTreeMap;

use crate::native::Workspace;
use anyhow::Result;
use noosphere::data::ContentType;
use noosphere_storage::memory::MemoryStore;

pub fn status_section(
    name: &str,
    entries: &BTreeMap<String, Option<ContentType>>,
    section: &mut Vec<(String, String, String)>,
    max_name_length: &mut usize,
    max_content_type_length: &mut usize,
) {
    for (slug, content_type) in entries {
        let content_type = content_type
            .as_ref()
            .map(|content_type| content_type.to_string())
            .unwrap_or_else(|| "Unknown".into());

        *max_name_length = *max_name_length.max(&mut slug.len());
        *max_content_type_length = *max_content_type_length.max(&mut content_type.len());

        section.push((slug.to_string(), content_type, String::from(name)));
    }
}

pub async fn status(workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let mut memory_store = MemoryStore::default();
    let db = workspace.get_local_db().await?;

    let (_, mut changes) = workspace
        .get_local_content_changes(None, &db, &mut memory_store)
        .await?;

    if changes.is_empty() {
        println!("No new changes to sphere content!");
        return Ok(());
    }

    let mut content = Vec::new();

    let mut max_name_length = 7usize;
    let mut max_content_type_length = 16usize;

    status_section(
        "Updated",
        &mut changes.updated,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "New",
        &mut changes.new,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "Removed",
        &mut changes.removed,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    println!(
        "{:max_name_length$}  {:max_content_type_length$}  STATUS",
        "NAME", "CONTENT-TYPE"
    );

    for (slug, content_type, status) in content {
        println!(
            "{:max_name_length$}  {:max_content_type_length$}  {}",
            slug, content_type, status
        );
    }

    Ok(())
}
