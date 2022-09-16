use anyhow::{anyhow, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::get_service, Router};
use noosphere_fs::SphereFs;
use noosphere_into::{html::sphere_into_html, write::NativeFs};
use std::{net::SocketAddr, os::unix::prelude::OsStrExt, path::Path};
use temp_dir::TempDir;
use tokio::fs::{self, File};
use tower_http::services::ServeDir;

use noosphere::{
    authority::generate_ed25519_key,
    data::{ContentType, Header, ReferenceIpld},
    view::Sphere,
};
use noosphere_storage::{interface::KeyValueStore, memory::MemoryStore};
use ucan::crypto::KeyMaterial;

pub async fn main() -> Result<()> {
    let mut sphere_store = MemoryStore::default();
    let mut block_store = MemoryStore::default();

    let owner_key = generate_ed25519_key();
    let owner_did = owner_key.get_did().await?;

    let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut block_store).await?;

    let sphere_identity = sphere.try_get_identity().await?;

    sphere_store
        .set(
            &sphere_identity,
            &ReferenceIpld {
                link: sphere.cid().clone(),
            },
        )
        .await?;

    let content_root = std::env::current_dir()?.join(Path::new("examples/notes-to-html/content"));
    let html_root = TempDir::new()?;

    println!("Content root: {:?}", content_root);
    println!("HTML root: {:?}", html_root.path());

    let mut sphere_fs = SphereFs::latest(&sphere_identity, &block_store, &sphere_store).await?;

    let mut read_dir = fs::read_dir(content_root).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        if let Some(extension) = entry.path().extension() {
            if extension != "subtext" {
                println!("Skipping non-subtext file: {:?}", entry.file_name());
                continue;
            }
        }

        let file_path = entry.path();
        let slug = std::str::from_utf8(
            file_path
                .file_stem()
                .ok_or_else(|| anyhow!("No slug able to be derived for {:?}", entry.file_name()))?
                .as_bytes(),
        )?;
        let file = File::open(&file_path).await?;
        let title = capitalize(&slug);

        sphere_fs
            .write(
                slug,
                &ContentType::Subtext.to_string(),
                file,
                &owner_key,
                Some(&proof),
                Some(vec![(Header::Title.to_string(), title)]),
            )
            .await?;
    }

    let native_fs = NativeFs {
        root: html_root.path().to_path_buf(),
    };

    sphere_into_html(&sphere_identity, &sphere_store, &block_store, &native_fs).await?;

    let app = Router::new()
        .fallback(get_service(ServeDir::new(html_root.path())).handle_error(handle_error));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Serving generated HTML at http://127.0.0.1:3000/");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn handle_error(_in: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
