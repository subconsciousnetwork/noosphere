![API Stability: Alpha](https://img.shields.io/badge/API%20Stability-Alpha-red)

# Noosphere Storage

The Rust implementation of Noosphere supports pluggable backing storage. This
crate defines the trait that must be implemented by a storage implementation,
and also contains ready-to-use implementations for native file storage (backed
by Sled), in-memory storage and web browser storage (backed by IndexedDB).
