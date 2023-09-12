#[allow(missing_docs)]
#[cfg(not(target_arch = "wasm32"))]
pub trait ConditionalSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> ConditionalSend for S where S: Send {}

#[allow(missing_docs)]
#[cfg(not(target_arch = "wasm32"))]
pub trait ConditionalSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> ConditionalSync for S where S: Send + Sync {}

#[allow(missing_docs)]
#[cfg(target_arch = "wasm32")]
pub trait ConditionalSend {}

#[cfg(target_arch = "wasm32")]
impl<S> ConditionalSend for S {}

#[allow(missing_docs)]
#[cfg(target_arch = "wasm32")]
pub trait ConditionalSync {}

#[cfg(target_arch = "wasm32")]
impl<S> ConditionalSync for S {}
