use noosphere_core::data::{Did, Mnemonic};

/// The result of creating a sphere is a [SphereReceipt], which reports both the
/// sphere identity (it's [Did]) and a [Mnemonic] which must be saved in some
/// secure storage medium on the side (it will not be recorded in the user's
/// sphere data).
pub struct SphereReceipt {
    /// The identity of the newly created sphere
    pub identity: Did,
    /// The recovery [Mnemonic] of the newly created sphere
    pub mnemonic: Mnemonic,
}
