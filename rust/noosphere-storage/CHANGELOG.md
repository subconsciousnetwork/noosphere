# Changelog

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.1.0...noosphere-storage-v0.2.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.

### Features

* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/subconsciousnetwork/noosphere/issues/162)) ([1e83bbb](https://github.com/subconsciousnetwork/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/subconsciousnetwork/noosphere/issues/177)) ([e269e04](https://github.com/subconsciousnetwork/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.1.0-alpha.1...noosphere-storage-v0.1.0) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
