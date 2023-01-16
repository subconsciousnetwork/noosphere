use anyhow::anyhow;
use itertools::Itertools;
use noosphere_core::data::Did;
use noosphere_fs::{SphereFile, SphereFs};
use safer_ffi::prelude::*;
use std::{pin::Pin, str::FromStr};
use subtext::{Peer, Slashlink};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{
    ffi::{NsHeaders, NsNoosphereContext},
    platform::{PlatformKeyMaterial, PlatformStorage},
};

ReprC! {
    #[ReprC::opaque]
    pub struct NsSphereFs {
        inner: SphereFs<PlatformStorage, PlatformKeyMaterial>
    }
}

impl NsSphereFs {
    pub fn inner(&self) -> &SphereFs<PlatformStorage, PlatformKeyMaterial> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut SphereFs<PlatformStorage, PlatformKeyMaterial> {
        &mut self.inner
    }
}

ReprC! {
    #[ReprC::opaque]
    pub struct NsSphereFile {
        inner: SphereFile<Pin<Box<dyn AsyncRead>>>
    }
}

impl NsSphereFile {
    pub fn inner(&self) -> &SphereFile<Pin<Box<dyn AsyncRead>>> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut SphereFile<Pin<Box<dyn AsyncRead>>> {
        &mut self.inner
    }
}

#[ffi_export]
/// Initialize an instance of a [NsSphereFs] that is a read/write view into
/// the contents (as addressed by the slug namespace) of the identitifed sphere
/// This will fail if it is not possible to initialize a sphere with the given
/// identity (which implies that no such sphere was ever created or joined on
/// this device).
pub fn ns_sphere_fs_open(
    noosphere: &NsNoosphereContext,
    sphere_identity: char_p::Ref<'_>,
) -> repr_c::Box<NsSphereFs> {
    noosphere
        .async_runtime()
        .block_on(async {
            let sphere_context = noosphere
                .inner()
                .get_sphere_context(&Did(sphere_identity.to_str().into()))
                .await?;

            let sphere_context = sphere_context.lock().await;

            Ok(repr_c::Box::new(NsSphereFs {
                inner: sphere_context.fs().await?,
            })) as Result<_, anyhow::Error>
        })
        .unwrap()
}

#[ffi_export]
/// De-allocate an [NsSphereFs] instance
pub fn ns_sphere_fs_free(sphere_fs: repr_c::Box<NsSphereFs>) {
    drop(sphere_fs)
}

#[ffi_export]
/// Read a [NsSphereFile] from a [NsSphereFs] instance by slashlink. Note that
/// although this function will eventually support slashlinks that include the
/// pet name of a peer, at this time only slashlinks with slugs referencing the
/// slug namespace of the local sphere are allowed.
///
/// This function will return a null pointer if the slug does not have a file
/// associated with it at the revision of the sphere that is referred to by the
/// [NsSphereFs] being read from.
pub fn ns_sphere_fs_read(
    noosphere: &NsNoosphereContext,
    sphere_fs: &NsSphereFs,
    slashlink: char_p::Ref<'_>,
) -> Option<repr_c::Box<NsSphereFile>> {
    noosphere
        .async_runtime()
        .block_on(async {
            let slashlink = match Slashlink::from_str(slashlink.to_str()) {
                Ok(slashlink) => slashlink,
                _ => return Ok(None),
            };

            if Peer::None != slashlink.peer {
                return Err(anyhow!("Peer in slashlink not yet supported"));
            }

            let slug = match slashlink.slug {
                Some(slug) => slug,
                None => return Err(anyhow!("No slug specified in slashlink!")),
            };

            println!(
                "Reading sphere {} slug {}...",
                sphere_fs.inner().identity(),
                slug
            );

            let file = sphere_fs.inner().read(&slug).await?;

            Ok(file.map(|sphere_file| {
                repr_c::Box::new(NsSphereFile {
                    inner: sphere_file.boxed(),
                })
            }))
        })
        .unwrap()
}

#[ffi_export]
/// Write a byte buffer to a slug in the given [NsSphereFs] instance, assigning
/// its content-type header to the specified value. If additional headers are
/// specified, they will be appended to the list of headers in the memo that is
/// created for the content. If some content already exists at the specified
/// slug, it will be assigned to be the previous historical revision of the new
/// content.
///
/// Note that you must invoke [ns_sphere_fs_save] to commit one or more writes
/// to the sphere.
pub fn ns_sphere_fs_write(
    noosphere: &NsNoosphereContext,
    sphere_fs: &mut NsSphereFs,
    slug: char_p::Ref<'_>,
    content_type: char_p::Ref<'_>,
    bytes: c_slice::Ref<'_, u8>,
    additional_headers: Option<&NsHeaders>,
) {
    noosphere.async_runtime().block_on(async {
        let slug = slug.to_str();

        println!(
            "Writing sphere {} slug {}...",
            sphere_fs.inner().identity(),
            slug
        );

        match sphere_fs
            .inner_mut()
            .write(
                slug,
                content_type.to_str().try_into().unwrap(),
                bytes.as_ref(),
                additional_headers.map(|headers| headers.inner().clone()),
            )
            .await
        {
            Ok(_) => println!("Updated {:?}...", slug),
            Err(error) => println!("Sphere write failed: {}", error),
        }
    })
}

#[ffi_export]
/// Save any writes performed on the [NsSphereFs] instance. If additional
/// headers are specified, they will be appended to the headers in the memo that
/// is created to wrap the latest sphere revision.
///
/// This will fail if both no writes have been performed and no additional
/// headers were specified (in other words: no actual changes were made).
pub fn ns_sphere_fs_save(
    noosphere: &NsNoosphereContext,
    sphere_fs: &mut NsSphereFs,
    additional_headers: Option<&NsHeaders>,
) {
    match noosphere.async_runtime().block_on(
        sphere_fs
            .inner_mut()
            .save(additional_headers.map(|headers| headers.inner().clone())),
    ) {
        Ok(cid) => println!(
            "Saved sphere {}; new revision is {}",
            sphere_fs.inner().identity(),
            cid
        ),
        Err(error) => println!("Sphere save failed: {}", error),
    }
}

#[ffi_export]
/// De-allocate an [NsSphereFile] instance
pub fn ns_sphere_file_free(sphere_file: repr_c::Box<NsSphereFile>) {
    drop(sphere_file)
}

#[ffi_export]
/// Read the contents of an [NsSphereFile] as a byte array. Note that the
/// [NsSphereFile] is lazy and stateful: it doesn't read any bytes from disk
/// until this function is invoked, and once the bytes have been read from the
/// file you must create a new [NsSphereFile] instance to read them again.
pub fn ns_sphere_file_contents_read(
    noosphere: &NsNoosphereContext,
    sphere_file: &mut NsSphereFile,
) -> c_slice::Box<u8> {
    noosphere.async_runtime().block_on(async {
        let mut buffer = Vec::new();

        sphere_file
            .inner_mut()
            .contents
            .read_to_end(&mut buffer)
            .await
            .unwrap();

        buffer.into_boxed_slice().into()
    })
}

#[ffi_export]
/// Read all header values for a file that correspond to a given name, returning
/// them as an array of strings
pub fn ns_sphere_file_header_values_read(
    sphere_file: &NsSphereFile,
    name: char_p::Ref<'_>,
) -> c_slice::Box<char_p::Box> {
    sphere_file
        .inner
        .memo
        .get_header(name.to_str())
        .into_iter()
        .map(|header| header.try_into().unwrap())
        .collect::<Vec<char_p::Box>>()
        .into_boxed_slice()
        .into()
}

#[ffi_export]
/// Get the first header value for a given name in the file, if any
pub fn ns_sphere_file_header_value_first(
    sphere_file: &NsSphereFile,
    name: char_p::Ref<'_>,
) -> Option<char_p::Box> {
    sphere_file
        .inner
        .memo
        .get_first_header(name.to_str())
        .into_iter()
        .nth(0)
        .map(|value| value.try_into().unwrap())
}

#[ffi_export]
/// Read all the headers associated with a file as an array of strings. Note
/// that headers will be reduced to a single entry in cases where multiple
/// headers share the same name.
pub fn ns_sphere_file_header_names_read(sphere_file: &NsSphereFile) -> c_slice::Box<char_p::Box> {
    sphere_file
        .inner
        .memo
        .headers
        .iter()
        .map(|(name, _)| name)
        .unique()
        .map(|name| name.to_owned().try_into().unwrap())
        .collect::<Vec<char_p::Box>>()
        .into_boxed_slice()
        .into()
}
