use std::pin::Pin;

use noosphere_core::data::{Did, Link, MemoIpld};
use tokio::io::AsyncRead;

#[cfg(not(target_arch = "wasm32"))]
pub trait AsyncFileBody: AsyncRead + Unpin + Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> AsyncFileBody for S where S: AsyncRead + Unpin + Send {}

#[cfg(target_arch = "wasm32")]
pub trait AsyncFileBody: AsyncRead + Unpin {}

#[cfg(target_arch = "wasm32")]
impl<S> AsyncFileBody for S where S: AsyncRead + Unpin {}

/// A descriptor for contents that is stored in a sphere.
pub struct SphereFile<C> {
    pub sphere_identity: Did,
    pub sphere_version: Link<MemoIpld>,
    pub memo_version: Link<MemoIpld>,
    pub memo: MemoIpld,
    pub contents: C,
}

impl<C> SphereFile<C>
where
    C: AsyncFileBody + 'static,
{
    pub fn boxed(self) -> SphereFile<Pin<Box<dyn AsyncFileBody + 'static>>> {
        SphereFile {
            sphere_identity: self.sphere_identity,
            sphere_version: self.sphere_version,
            memo_version: self.memo_version,
            memo: self.memo,
            contents: Box::pin(self.contents),
        }
    }
}
