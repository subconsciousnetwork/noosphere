// See: https://github.com/getditto/safer_ffi/issues/31#issuecomment-782070270
#![allow(improper_ctypes_definitions)]

use anyhow::anyhow;
use cid::Cid;
use itertools::Itertools;
use noosphere_core::data::Did;
use safer_ffi::{char_p::InvalidNulTerminator, prelude::*};
use std::{os::raw::c_void, pin::Pin, str::FromStr, sync::Arc};
use subtext::{Peer, Slashlink};
use tokio::{io::AsyncReadExt, sync::Mutex};

use crate::{
    error::NoosphereError,
    ffi::{NsError, NsHeaders, NsNoosphere, TryOrInitialize},
    platform::{PlatformKeyMaterial, PlatformSphereChannel, PlatformStorage},
};

use noosphere_sphere::{
    AsyncFileBody, HasMutableSphereContext, HasSphereContext, SphereContentRead,
    SphereContentWrite, SphereContext, SphereCursor, SphereFile, SphereReplicaRead, SphereWalker,
};

#[derive_ReprC(rename = "ns_sphere")]
#[repr(opaque)]
/// @class ns_sphere_t
///
/// An opaque struct representing a sphere.
pub struct NsSphere {
    inner: PlatformSphereChannel,
}

impl NsSphere {
    pub fn inner(
        &self,
    ) -> &SphereCursor<
        Arc<SphereContext<PlatformKeyMaterial, PlatformStorage>>,
        PlatformKeyMaterial,
        PlatformStorage,
    > {
        self.inner.immutable()
    }

    pub fn inner_mut(
        &mut self,
    ) -> &mut Arc<Mutex<SphereContext<PlatformKeyMaterial, PlatformStorage>>> {
        self.inner.mutable()
    }

    pub fn to_channel(&self) -> PlatformSphereChannel {
        self.inner.clone()
    }
}

#[derive_ReprC(rename = "ns_sphere_file")]
#[repr(opaque)]
/// @class ns_sphere_file_t
///
/// A read/write view into a sphere's memo.
///
/// ns_sphere_file_t is a lazy, stateful view into a single memo.
/// No bytes are read from disk until ns_sphere_file_contents_read() is invoked.
pub struct NsSphereFile {
    inner: SphereFile<Pin<Box<dyn AsyncFileBody>>>,
}

impl NsSphereFile {
    pub fn inner(&self) -> &SphereFile<Pin<Box<dyn AsyncFileBody>>> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut SphereFile<Pin<Box<dyn AsyncFileBody>>> {
        &mut self.inner
    }
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Initialize an ns_sphere_t instance.
///
/// This will fail if it is not possible to initialize a sphere with the given
/// identity (which implies that no such sphere was ever created or joined on
/// this device).
pub fn ns_sphere_open(
    noosphere: &NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsSphere>> {
    error_out.try_or_initialize(|| {
        let fs = noosphere.async_runtime().block_on(async {
            let sphere_channel = noosphere
                .inner()
                .get_sphere_channel(&Did(sphere_identity.to_str().into()))
                .await?;

            Ok(Box::new(NsSphere {
                inner: sphere_channel,
            })
            .into()) as Result<_, anyhow::Error>
        })?;

        Ok(fs)
    })
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Deallocate an ns_sphere_t instance.
pub fn ns_sphere_free(sphere: repr_c::Box<NsSphere>) {
    drop(sphere)
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Access another sphere by a petname.
///
/// The petname should be one that has been assigned to the sphere's identity
/// using ns_sphere_petname_set(). If any of the data required to access the
/// target sphere is not available locally, it will be replicated from the
/// network through the configured Noosphere Gateway. If no such gateway is
/// configured and the data is not available locally, this call will fail. The
/// returned ns_sphere_t pointer can be used to access the content, petnames,
/// revision history and other features of the target sphere with the same APIs
/// used to access the local user's sphere, except that any operations that
/// attempt to modify the sphere will be rejected.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_traverse_by_petname
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to an ns_sphere_t if the call was successful, otherwise
///     NULL
///
/// The traversal can be made recursive by chaining together petnames with a '.'
/// as a delimiter. The name traversal will be from back to front, so if you
/// traverse to the name "bob.alice.carol" it will first traverse to "carol",
/// then to carol's "alice", then to carol's alice's "bob."
///
/// Note that this function has a reasonable likelihood to call out to the
/// network, notably in cases where a petname is assigned to an identity but the
/// sphere data is not available to local storage.
#[allow(clippy::type_complexity)]
pub fn ns_sphere_traverse_by_petname(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    petname: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<repr_c::Box<NsSphere>>,
    ),
) {
    let sphere = sphere.inner().clone();
    let async_runtime = noosphere.async_runtime();
    let raw_petnames = format!("@{}", petname.to_str());

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let link = Slashlink::from_str(&raw_petnames)?;
            let petnames = match link.peer {
                Peer::Name(petnames) => petnames,
                _ => Err(anyhow!("No petnames found in {}", raw_petnames))?,
            };

            let next_sphere_context = sphere.traverse_by_petnames(&petnames).await?;

            Ok(next_sphere_context.map(|next_sphere_context| {
                Box::new(NsSphere {
                    inner: next_sphere_context.into(),
                })
                .into()
            })) as Result<Option<_>, anyhow::Error>
        }
        .await;

        match result {
            Ok(maybe_sphere) => {
                async_runtime.spawn_blocking(move || callback(context, None, maybe_sphere))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// @deprecated Blocking FFI is deprecated, use ns_sphere_traverse_by_petname
/// instead
///
/// Same as ns_sphere_traverse_by_petname, but blocks the current thread while
/// performing its work.
pub fn ns_sphere_traverse_by_petname_blocking(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsSphere>> {
    match error_out.try_or_initialize(|| {
        let sphere = noosphere.async_runtime().block_on(async {
            let raw_petnames = petname.to_str();
            let link = Slashlink::from_str(&format!("@{raw_petnames}"))?;
            let petnames = match link.peer {
                Peer::Name(petnames) => petnames,
                _ => return Err(anyhow!("No petnames found in {}", raw_petnames)),
            };

            let next_sphere_context = sphere.inner().traverse_by_petnames(&petnames).await?;

            Ok(next_sphere_context.map(|next_sphere_context| {
                Box::new(NsSphere {
                    inner: next_sphere_context.into(),
                })
                .into()
            })) as Result<Option<_>, anyhow::Error>
        })?;

        Ok(sphere)
    }) {
        Some(maybe_sphere) => maybe_sphere,
        None => None,
    }
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Read a memo as a ns_sphere_file_t from a ns_sphere_t by slashlink.
///
/// This function supports slashlinks that contain only a slug component or with
/// both a slug and a peer component.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_content_read
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to an ns_sphere_file_t if the call was successful,
///     otherwise NULL
///
/// Note that although this function will eventually support slashlinks that use
/// a raw DID as the peer, it is not supported at this time and trying to read
/// from such a link will fail with an error.
///
/// This function will return a null pointer if the slug does not have a file
/// associated with it at the revision of the sphere that is referred to by the
/// ns_sphere_t being read from.
#[allow(clippy::type_complexity)]
pub fn ns_sphere_content_read(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    slashlink: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<repr_c::Box<NsSphereFile>>,
    ),
) {
    let sphere = sphere.inner().clone();
    let slashlink = slashlink.to_string();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let slashlink = Slashlink::from_str(&slashlink)?;

            let slug = match slashlink.slug {
                Some(slug) => slug,
                None => return Err(anyhow!("No slug specified in slashlink!")),
            };

            let cursor = match slashlink.peer {
                Peer::Name(petnames) => match sphere.traverse_by_petnames(&petnames).await? {
                    Some(sphere_context) => sphere_context,
                    None => return Ok(None),
                },
                Peer::None => sphere,
                Peer::Did(_) => return Err(anyhow!("DID peer in slashlink not yet supported")),
            };

            info!(
                "Reading sphere {} slug {}...",
                cursor.identity().await?,
                slug
            );

            let file = cursor.read(&slug).await?;

            Ok(file.map(|sphere_file| {
                Box::new(NsSphereFile {
                    inner: sphere_file.boxed(),
                })
                .into()
            }))
        }
        .await;

        match result {
            Ok(maybe_file) => {
                async_runtime.spawn_blocking(move || callback(context, None, maybe_file))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// @deprecated Blocking FFI is deprecated, use ns_sphere_content_read instead
///
/// Same as ns_sphere_content_read, but blocks the current thread while
/// performing its work.
pub fn ns_sphere_content_read_blocking(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    slashlink: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsSphereFile>> {
    match error_out.try_or_initialize(|| {
        noosphere
            .async_runtime()
            .block_on(async {
                let slashlink = Slashlink::from_str(slashlink.to_str())?;

                let slug = match slashlink.slug {
                    Some(slug) => slug,
                    None => return Err(anyhow!("No slug specified in slashlink!")),
                };

                let cursor = match slashlink.peer {
                    Peer::Name(petnames) => {
                        match sphere.inner().traverse_by_petnames(&petnames).await? {
                            Some(sphere_context) => sphere_context,
                            None => return Ok(None),
                        }
                    }
                    Peer::None => sphere.inner().clone(),
                    Peer::Did(_) => return Err(anyhow!("DID peer in slashlink not yet supported")),
                };

                info!(
                    "Reading sphere {} slug {}...",
                    cursor.identity().await?,
                    slug
                );

                let file = cursor.read(&slug).await?;

                Ok(file.map(|sphere_file| {
                    Box::new(NsSphereFile {
                        inner: sphere_file.boxed(),
                    })
                    .into()
                }))
            })
            .map_err(|error| error.into())
    }) {
        Some(maybe_file) => maybe_file,
        None => None,
    }
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Write content to a ns_sphere_t instance, keyed by `slug`, assigning its
/// content-type header to the specified value.
///
/// If additional headers are specified, they will be appended to the list
/// of headers in the memo that is created for the content. If some content
/// already exists at the specified slug, it will be assigned to be the
/// previous historical revision of the new content.
///
/// Note that you must invoke ns_sphere_save() to commit one or more writes
/// to the sphere.
pub fn ns_sphere_content_write(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    slug: char_p::Ref<'_>,
    content_type: char_p::Ref<'_>,
    bytes: c_slice::Ref<'_, u8>,
    additional_headers: Option<&NsHeaders>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let slug = slug.to_str();
            let mut cursor = SphereCursor::latest(sphere.inner_mut().clone());

            info!(
                "Writing sphere {} slug {}...",
                cursor.identity().await?,
                slug
            );

            cursor
                .write(
                    slug,
                    content_type.to_str(),
                    bytes.as_ref(),
                    additional_headers.map(|headers| headers.inner().clone()),
                )
                .await?;

            println!("Updated {slug:?}...");

            Ok(())
        })
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Unlinks a slug from the content space.
///
/// Note that this does not remove the blocks that were previously associated
/// with the content found at the given slug, because they will still be
/// available at an earlier revision of the sphere. In order to commit the
/// change, you must save. Note that this call is a no-op if there is
/// no matching slug linked in the sphere.
pub fn ns_sphere_content_remove(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    slug: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        noosphere
            .async_runtime()
            .block_on(async { sphere.inner_mut().remove(slug.to_str()).await })?;
        Ok(())
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Save any writes performed on the ns_sphere_t instance.
///
/// If additional headers are specified, they will be appended to
/// the headers in the memo that is created to wrap the latest sphere revision.
pub fn ns_sphere_save(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    additional_headers: Option<&NsHeaders>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        let cid = noosphere.async_runtime().block_on(
            sphere
                .inner_mut()
                .save(additional_headers.map(|headers| headers.inner().clone())),
        )?;

        println!("Saved sphere; new revision is {cid}");

        Ok(())
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Get an array of all of the slugs in a sphere at the current version.
pub fn ns_sphere_content_list(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> c_slice::Box<char_p::Box> {
    let possible_output = error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let slug_set = SphereWalker::from(sphere.inner().clone())
                .list_slugs()
                .await?;
            let mut all_slugs: Vec<char_p::Box> = Vec::new();

            for slug in slug_set.into_iter() {
                all_slugs.push(
                    slug.try_into()
                        .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?,
                );
            }

            Ok(all_slugs)
        })
    });

    match possible_output {
        Some(slugs) => slugs,
        None => Vec::new(),
    }
    .into_boxed_slice()
    .into()
}

#[ffi_export]
/// @memberof ns_sphere_t
/// Get an array of all of the slugs that changed in a given sphere since a
/// given revision of that sphere (excluding the given revision).
///
/// The revision should be provided as a base64-encoded CID v1 string.
/// If no revision is provided, the entire history will be considered,
/// back to and including the first revision.
///
/// Note that a slug change may mean the slug was added, updated or removed.
/// Also note that multiple changes to the same slug will be reduced to a
/// single entry in the array that is returned.
pub fn ns_sphere_content_changes(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    since_cid: Option<char_p::Ref<'_>>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> c_slice::Box<char_p::Box> {
    let possible_output = error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let since = match since_cid {
                Some(cid_string) => Some(
                    Cid::from_str(cid_string.to_str())
                        .map_err(|error| anyhow!(error))?
                        .into(),
                ),
                None => None,
            };

            let changed_slug_set = SphereWalker::from(sphere.inner().clone())
                .content_changes(since.as_ref())
                .await?;
            let mut changed_slugs: Vec<char_p::Box> = Vec::new();

            for slug in changed_slug_set.into_iter() {
                changed_slugs.push(
                    slug.try_into()
                        .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?,
                );
            }

            Ok(changed_slugs)
        })
    });

    match possible_output {
        Some(slugs) => slugs,
        None => Vec::new(),
    }
    .into_boxed_slice()
    .into()
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// Deallocate a ns_sphere_file_t instance.
pub fn ns_sphere_file_free(sphere_file: repr_c::Box<NsSphereFile>) {
    drop(sphere_file)
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// Trade in an ns_sphere_file_t for the bytes of the file contents it refers
/// to. Note that the implication here is that bytes can only be read from a
/// ns_sphere_file_t one time; if you need to read them multiple times, you
/// should call ns_sphere_content_read each time.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_file_contents_read
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to a sized byte array (slice_boxed_uint8_t) if the call
///     was successful, otherwise NULL
///
#[allow(clippy::type_complexity)]
pub fn ns_sphere_file_contents_read(
    noosphere: &NsNoosphere,
    mut sphere_file: repr_c::Box<NsSphereFile>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<c_slice::Box<u8>>,
    ),
) {
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let mut buffer = Vec::new();

            sphere_file
                .inner_mut()
                .contents
                .read_to_end(&mut buffer)
                .await
                .map_err(|error| anyhow!(error))?;

            Ok(buffer.into_boxed_slice().into()) as Result<_, anyhow::Error>
        }
        .await;

        match result {
            Ok(maybe_bytes) => {
                async_runtime.spawn_blocking(move || callback(context, None, Some(maybe_bytes)))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// @deprecated Blocking FFI is deprecated, use ns_sphere_file_contents_read
/// instead
///
/// Same as ns_sphere_file_contents_read, but blocks the current thread while
/// performing its work.
pub fn ns_sphere_file_contents_read_blocking(
    noosphere: &NsNoosphere,
    sphere_file: &mut NsSphereFile,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<c_slice::Box<u8>> {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let mut buffer = Vec::new();

            sphere_file
                .inner_mut()
                .contents
                .read_to_end(&mut buffer)
                .await
                .map_err(|error| anyhow!(error))?;

            Ok(buffer.into_boxed_slice().into())
        })
    })
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
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
        .filter_map(|header| header.try_into().ok())
        .collect::<Vec<char_p::Box>>()
        .into_boxed_slice()
        .into()
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// Get the first header value for a given name in the file, if any.
pub fn ns_sphere_file_header_value_first(
    sphere_file: &NsSphereFile,
    name: char_p::Ref<'_>,
) -> Option<char_p::Box> {
    sphere_file
        .inner
        .memo
        .get_first_header(name.to_str())
        .into_iter()
        .filter_map(|value| value.try_into().ok())
        .next()
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// Read all the headers associated with a file as an array of strings.
///
/// The headers will be reduced to a single entry in cases where multiple
/// headers share the same name.
pub fn ns_sphere_file_header_names_read(sphere_file: &NsSphereFile) -> c_slice::Box<char_p::Box> {
    sphere_file
        .inner
        .memo
        .headers
        .iter()
        .map(|(name, _)| name)
        .unique()
        .filter_map(|name| name.to_owned().try_into().ok())
        .collect::<Vec<char_p::Box>>()
        .into_boxed_slice()
        .into()
}

#[ffi_export]
/// @memberof ns_sphere_file_t
///
/// Get the base64-encoded CID v1 string for the memo that refers to the content
/// of this ns_sphere_file_t.
pub fn ns_sphere_file_version_get(
    sphere_file: &NsSphereFile,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        sphere_file
            .inner
            .memo_version
            .to_string()
            .try_into()
            .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
    })
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Get the identity (a DID encoded as a UTF-8 string)
/// for this ns_sphere_t.
pub fn ns_sphere_identity(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        match noosphere
            .async_runtime()
            .block_on(async { sphere.inner().identity().await })
        {
            Ok(identity) => identity
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into()),
            Err(error) => Err(anyhow!(error).into()),
        }
    })
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Get the version (a CID encoded as a UTF-8 string) for this ns_sphere_t.
pub fn ns_sphere_version(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        match noosphere
            .async_runtime()
            .block_on(async { sphere.inner().version().await })
        {
            Ok(version) => version
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into()),
            Err(error) => Err(anyhow!(error).into()),
        }
    })
}
