use std::pin::Pin;

use cid::Cid;
use noosphere_core::data::{Did, MemoIpld};
use tokio::io::AsyncRead;

/// A descriptor for contents that is stored in a sphere.
pub struct SphereFile<C> {
    pub sphere_identity: Did,
    pub sphere_revision: Cid,
    pub memo_revision: Cid,
    pub memo: MemoIpld,
    pub contents: C,
}

impl<C> SphereFile<C>
where
    C: AsyncRead + 'static,
{
    pub fn boxed(self) -> SphereFile<Pin<Box<dyn AsyncRead + 'static>>> {
        SphereFile {
            sphere_identity: self.sphere_identity,
            sphere_revision: self.sphere_revision,
            memo_revision: self.memo_revision,
            memo: self.memo,
            contents: Box::pin(self.contents),
        }
    }
}
