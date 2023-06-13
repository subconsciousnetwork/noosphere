use std::pin::Pin;

use noosphere_core::data::{Did, Link, MemoIpld};
use tokio::io::AsyncRead;

/// A type that may be used as the contents field in a [SphereFile]
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
    /// The identity of the associated sphere from which the file was read
    pub sphere_identity: Did,
    /// The version of the associated sphere from which the file was read
    pub sphere_version: Link<MemoIpld>,
    /// The version of the memo that wraps the file's body contents
    pub memo_version: Link<MemoIpld>,
    /// The memo that wraps the file's body contents
    pub memo: MemoIpld,
    /// The body contents of the file
    pub contents: C,
}

impl<C> SphereFile<C>
where
    C: AsyncFileBody + 'static,
{
    /// Consume the file and return a version of it where its body contents have
    /// been boxed and pinned
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
