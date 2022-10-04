use anyhow::Result;


use crate::native::workspace::Workspace;

pub async fn auth_add(_did: &str, name: Option<String>, workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let _name = match name {
        Some(name) => name,
        None => {
            let random_name = witty_phrase_generator::WPGen::new()
                .with_words(3)
                .unwrap_or_else(|| vec!["Unnamed"])
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>()
                .join("-");
            println!(
                "Note: since no name was specified, the authorization will be saved with the pet name \"{}\"",
                random_name
            );
            random_name
        }
    };

    // let key_material = workspace.get_local_key().await?;
    // let sphere =

    // let ucan = UcanBuilder::default().issued_by(&key_material).for_audience(did).claiming_capability(capability)

    Ok(())
}
