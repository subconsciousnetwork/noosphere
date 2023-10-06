# Changelog

## [0.9.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.9.0...noosphere-storage-v0.9.1) (2023-10-06)


### Features

* Improved IPFS Kubo syndication ([#666](https://github.com/subconsciousnetwork/noosphere/issues/666)) ([eeab932](https://github.com/subconsciousnetwork/noosphere/commit/eeab932763cd642702bc6ac85a6bbc10968a107d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-common bumped from 0.1.0 to 0.1.1

## [0.9.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.8.1...noosphere-storage-v0.9.0) (2023-09-19)


### ⚠ BREAKING CHANGES

* Disaster recovery via gateway ([#637](https://github.com/subconsciousnetwork/noosphere/issues/637))
* Replace `Bundle` with CAR streams in push ([#624](https://github.com/subconsciousnetwork/noosphere/issues/624))

### Features

* Disaster recovery via gateway ([#637](https://github.com/subconsciousnetwork/noosphere/issues/637)) ([70e7331](https://github.com/subconsciousnetwork/noosphere/commit/70e7331767f65e0976ee5843229f765dc6ace7fb))
* Introduce RocksDbStorage, genericize storage throughout. ([#623](https://github.com/subconsciousnetwork/noosphere/issues/623)) ([7155f86](https://github.com/subconsciousnetwork/noosphere/commit/7155f860c2f5ee481d923941dca95981cd7f4b38))
* Replace `Bundle` with CAR streams in push ([#624](https://github.com/subconsciousnetwork/noosphere/issues/624)) ([9390797](https://github.com/subconsciousnetwork/noosphere/commit/9390797eb6653fdecd41c3a54225ffd55945bb89))

## [0.8.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.8.0...noosphere-storage-v0.8.1) (2023-08-10)


### Features

* `orb sphere history` and `orb sphere render` ([#576](https://github.com/subconsciousnetwork/noosphere/issues/576)) ([a6f0a74](https://github.com/subconsciousnetwork/noosphere/commit/a6f0a74cde2fc001bfff5c1bed0844ac19fc8258))
* flush when dropping NativeStorage to prevent intermittent failures. ([#580](https://github.com/subconsciousnetwork/noosphere/issues/580)) ([76f678f](https://github.com/subconsciousnetwork/noosphere/commit/76f678fed59dcfb56360927d3a6a352f90c020ee))

## [0.8.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.7.1...noosphere-storage-v0.8.0) (2023-08-04)


### ⚠ BREAKING CHANGES

* `orb` uses latest Noosphere capabilities ([#530](https://github.com/subconsciousnetwork/noosphere/issues/530))

### Features

* `orb` uses latest Noosphere capabilities ([#530](https://github.com/subconsciousnetwork/noosphere/issues/530)) ([adfa028](https://github.com/subconsciousnetwork/noosphere/commit/adfa028ebcb2de7ea7492af57239fcc9bfc27955))
* Synchronously flush sled DB rather than using sled::Tree::flush_async, which can cause deadlocks. Fixes [#403](https://github.com/subconsciousnetwork/noosphere/issues/403) ([#540](https://github.com/subconsciousnetwork/noosphere/issues/540)) ([7262d5c](https://github.com/subconsciousnetwork/noosphere/commit/7262d5c884bf756b70a8d115d75a5e11e98c9a54))

## [0.7.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.7.0...noosphere-storage-v0.7.1) (2023-07-01)


### Features

* Featureful documentation ([78afa44](https://github.com/subconsciousnetwork/noosphere/commit/78afa4423ac797cc7eb6a0544d972049480fe153))

## [0.7.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.6.3...noosphere-storage-v0.7.0) (2023-06-08)


### ⚠ BREAKING CHANGES

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409))
* Migrate blake2b->blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400))

### Features

* Consolidate `NsRecord` implementation in`LinkRecord`. Fixes [#395](https://github.com/subconsciousnetwork/noosphere/issues/395) ([#399](https://github.com/subconsciousnetwork/noosphere/issues/399)) ([9ee4798](https://github.com/subconsciousnetwork/noosphere/commit/9ee47981232fde00b34bb9458c5b0b2799a610ca))
* Migrate blake2b-&gt;blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400)) ([f9e0aec](https://github.com/subconsciousnetwork/noosphere/commit/f9e0aecd76a7253aba13b1881af32a2e543fb6de)), closes [#386](https://github.com/subconsciousnetwork/noosphere/issues/386)


### Bug Fixes

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409)) ([8812a1e](https://github.com/subconsciousnetwork/noosphere/commit/8812a1e8c9348301b36b77d6c1a2024432806358))

## [0.6.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.6.2...noosphere-storage-v0.6.3) (2023-05-08)


### Features

* Enable expired yet valid records in the name system. Update to ucan 0.2.0. ([#360](https://github.com/subconsciousnetwork/noosphere/issues/360)) ([3b0663a](https://github.com/subconsciousnetwork/noosphere/commit/3b0663abc7783a6d33dd47d20caae7597ab93ed0))
* Make anyhow a workspace dependency ([721c994](https://github.com/subconsciousnetwork/noosphere/commit/721c994886228dc61941328e49f7c8928269cdb8))

## [0.6.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.6.1...noosphere-storage-v0.6.2) (2023-04-22)


### Features

* Update IPLD-related dependencies ([#327](https://github.com/subconsciousnetwork/noosphere/issues/327)) ([5fdfadb](https://github.com/subconsciousnetwork/noosphere/commit/5fdfadb1656f9d6eef2dbbb8b00a598106bccf00))

## [0.6.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.6.0...noosphere-storage-v0.6.1) (2023-04-10)


### Features

* Dot syntax when traversing by petname ([#306](https://github.com/subconsciousnetwork/noosphere/issues/306)) ([cd87b05](https://github.com/subconsciousnetwork/noosphere/commit/cd87b0533c21bbbd4d82332556e70ecc706a5531))

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.5.0...noosphere-storage-v0.6.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Traverse the Noosphere vast (#284)

### Features

* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.4.2...noosphere-storage-v0.5.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Petname resolution and synchronization in spheres and gateways (#253)
* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. (#254)
* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. (#252)

### Features

* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Bug Fixes

* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. ([#254](https://github.com/subconsciousnetwork/noosphere/issues/254)) ([b79872a](https://github.com/subconsciousnetwork/noosphere/commit/b79872afd54c7b69d447dfe99e750bb6a813645c))


### Miscellaneous Chores

* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. ([#252](https://github.com/subconsciousnetwork/noosphere/issues/252)) ([518beae](https://github.com/subconsciousnetwork/noosphere/commit/518beae563bd04c921ee3c6641a7249f14c611e4))

## [0.4.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.4.1...noosphere-storage-v0.4.2) (2023-02-16)


### Features

* Always flush on SphereFS save ([#231](https://github.com/subconsciousnetwork/noosphere/issues/231)) ([bd151d5](https://github.com/subconsciousnetwork/noosphere/commit/bd151d5aca75b78b786d008177ab7d4e53e843bc))

## [0.4.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.4.0...noosphere-storage-v0.4.1) (2023-01-19)


### Features

* Improvements to the NameSystem based on initial gateway integration ([#196](https://github.com/subconsciousnetwork/noosphere/issues/196)) ([4a6898e](https://github.com/subconsciousnetwork/noosphere/commit/4a6898e0aa8e1d96780226d384a6876eac122658))

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.3.0...noosphere-storage-v0.4.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-storage-v0.2.0...noosphere-storage-v0.3.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

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
