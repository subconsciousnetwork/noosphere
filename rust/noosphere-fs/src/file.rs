use cid::Cid;
use noosphere::data::MemoIpld;

/// A descriptor for contents that is stored in a sphere.
pub struct SphereFile<C> {
    pub sphere_revision: Cid,
    pub memo_revision: Cid,
    pub memo: MemoIpld,
    pub contents: C,
}
