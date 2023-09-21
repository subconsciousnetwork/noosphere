use anyhow::{anyhow, Result};
use axum::{error_handling::HandleErrorLayer, http::StatusCode, routing::get_service};
use noosphere_core::context::{
    HasMutableSphereContext, SphereContentWrite, SphereContext, SphereContextKey, SphereCursor,
};
use noosphere_into::{sphere_into_html, NativeFs};
use std::{ffi::OsStr, net::SocketAddr, path::Path, sync::Arc};
use tempfile::TempDir;
use tokio::{
    fs::{self, File},
    sync::Mutex,
};
use tower_http::services::ServeDir;

#[cfg(unix)]
use std::os::unix::prelude::OsStrExt;
#[cfg(windows)]
use std::os::windows::prelude::OsStrExt;

use noosphere_core::{
    authority::{generate_ed25519_key, Author},
    data::{ContentType, Header},
    view::Sphere,
};
use noosphere_storage::{MemoryStorage, SphereDb};
use ucan::crypto::KeyMaterial;

pub async fn main() -> Result<()> {
    let storage_provider = MemoryStorage::default();
    let mut db = SphereDb::new(storage_provider).await.unwrap();

    let owner_key: SphereContextKey = Arc::new(Box::new(generate_ed25519_key()));
    let owner_did = owner_key.get_did().await?;

    let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;

    let sphere_identity = sphere.get_identity().await.unwrap();
    let author = Author {
        key: owner_key,
        authorization: Some(proof),
    };

    db.set_version(&sphere_identity, sphere.cid()).await?;

    let context = Arc::new(Mutex::new(
        SphereContext::new(sphere_identity, author, db, None).await?,
    ));
    let mut cursor = SphereCursor::latest(context);

    let content_root = std::env::current_dir()?.join(Path::new("examples/notes-to-html/content"));
    let html_root = TempDir::new()?;

    println!("Content root: {:?}", content_root);
    println!("HTML root: {:?}", html_root.path());

    let mut read_dir = fs::read_dir(content_root).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        if let Some(extension) = entry.path().extension() {
            if extension != "subtext" {
                println!("Skipping non-subtext file: {:?}", entry.file_name());
                continue;
            }
        }

        let file_path = entry.path();
        let slug =
            os_str_to_str(file_path.file_stem().ok_or_else(|| {
                anyhow!("No slug able to be derived for {:?}", entry.file_name())
            })?)
            .map_err(|_| anyhow!("Could not parse slug into UTF-8"))?;

        let file = File::open(&file_path).await?;
        let title = capitalize(&slug);

        cursor
            .write(
                &slug,
                &ContentType::Subtext,
                file,
                Some(vec![(Header::Title.to_string(), title)]),
            )
            .await?;
    }

    cursor.save(None).await?;

    let native_fs = NativeFs {
        root: html_root.path().to_path_buf(),
    };

    sphere_into_html(cursor, &native_fs).await?;

    let app =
        get_service(ServeDir::new(html_root.path())).layer(HandleErrorLayer::new(|_| async {
            (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
        }));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Serving generated HTML at http://127.0.0.1:3000/");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn os_str_to_str(os_str: &OsStr) -> Result<String, std::str::Utf8Error> {
    #[cfg(unix)]
    let out = String::from(std::str::from_utf8(os_str.as_bytes())?);
    #[cfg(windows)]
    let out = String::from_utf16_lossy(&os_str.encode_wide().collect::<Vec<u16>>());

    Ok(out)
}
