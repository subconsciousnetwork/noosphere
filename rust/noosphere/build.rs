use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        // Platforms
        wasm: { target_arch = "wasm32" },
        native: { not(target_arch = "wasm32") },
        apple: {
            all(
                target_vendor = "apple",
                any(target_arch = "aarch64", target_arch = "x86_64")
            )
        },

        // Backends
        rocksdb: { all(feature = "rocksdb", native, not(sqlite)) },
        sqlite: { all(feature = "sqlite", native) },
        sled: { all(not(any(sqlite, rocksdb)), native) },
        indexeddb: { wasm },

        // Other
        ipfs_storage: { feature = "ipfs-storage" },
    }
}
