# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.2 to 0.4.3

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.4 to 0.4.5

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.6.0 to 0.7.0

## [0.8.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.8.1...noosphere-v0.8.2) (2023-04-10)


### Features

* Dot syntax when traversing by petname ([#306](https://github.com/subconsciousnetwork/noosphere/issues/306)) ([cd87b05](https://github.com/subconsciousnetwork/noosphere/commit/cd87b0533c21bbbd4d82332556e70ecc706a5531))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.7.0 to 0.7.1

## [0.8.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.7.0...noosphere-v0.8.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Sphere traversal C FFI (#292)
* Traverse the Noosphere vast (#284)

### Features

* Sphere traversal C FFI ([#292](https://github.com/subconsciousnetwork/noosphere/issues/292)) ([5d55e60](https://github.com/subconsciousnetwork/noosphere/commit/5d55e60789fcec6abdcc50df10f0038274972806))
* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.5.0 to 0.6.0

## [0.7.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.6.3...noosphere-v0.7.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Implement C FFI for petname management (#271)
* Petname resolution and synchronization in spheres and gateways (#253)
* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. (#254)
* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. (#252)

### Features

* Implement C FFI for petname management ([#271](https://github.com/subconsciousnetwork/noosphere/issues/271)) ([d43c628](https://github.com/subconsciousnetwork/noosphere/commit/d43c6283c6b2374de503d70bd46c8df7d0337c3a))
* Initial example of C integration. ([#242](https://github.com/subconsciousnetwork/noosphere/issues/242)) ([57beb24](https://github.com/subconsciousnetwork/noosphere/commit/57beb24f9996a92fa348657a58920a7944f53e05))
* Introduce `noosphere-gateway` crate ([#238](https://github.com/subconsciousnetwork/noosphere/issues/238)) ([791bc39](https://github.com/subconsciousnetwork/noosphere/commit/791bc3996cfac12cb077c3721f22d080a71d33ba))
* Noosphere builds and runs tests on Windows ([#228](https://github.com/subconsciousnetwork/noosphere/issues/228)) ([d1320f0](https://github.com/subconsciousnetwork/noosphere/commit/d1320f08429c8f8090fd4612b56ebf9386414cc7))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Bug Fixes

* FFI header generation: Use an ordered BTreeMap to replace class token keys so that class names that are subsets of other class names are replaced appropriately. ([#270](https://github.com/subconsciousnetwork/noosphere/issues/270)) ([4cf2e40](https://github.com/subconsciousnetwork/noosphere/commit/4cf2e4053c3caad3fc903d285c98b6ac459c9582))
* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. ([#254](https://github.com/subconsciousnetwork/noosphere/issues/254)) ([b79872a](https://github.com/subconsciousnetwork/noosphere/commit/b79872afd54c7b69d447dfe99e750bb6a813645c))


### Miscellaneous Chores

* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. ([#252](https://github.com/subconsciousnetwork/noosphere/issues/252)) ([518beae](https://github.com/subconsciousnetwork/noosphere/commit/518beae563bd04c921ee3c6641a7249f14c611e4))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.5 to 0.5.0

## [0.6.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.6.1...noosphere-v0.6.2) (2023-02-07)


### Features

* General error handling in C FFI ([#219](https://github.com/subconsciousnetwork/noosphere/issues/219)) ([0a1952b](https://github.com/subconsciousnetwork/noosphere/commit/0a1952b34895071d2203505c95750d453bb110c6))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.3 to 0.4.4

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.5.1...noosphere-v0.6.0) (2023-01-31)


### ⚠ BREAKING CHANGES

* Sphere sync and change diff in C FFI (#210)

### Features

* Sphere sync and change diff in C FFI ([#210](https://github.com/subconsciousnetwork/noosphere/issues/210)) ([306d39c](https://github.com/subconsciousnetwork/noosphere/commit/306d39cdf6727fbeb34a49740b55f56834f4df07))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.1 to 0.4.2

## [0.5.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.5.0...noosphere-v0.5.1) (2023-01-19)


### Features

* Extend C FFI for header enumeration ([#202](https://github.com/subconsciousnetwork/noosphere/issues/202)) ([b404ec0](https://github.com/subconsciousnetwork/noosphere/commit/b404ec0d117e2467bfbe4a3bda4253e1c57f584e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.0 to 0.4.1

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.4.0...noosphere-v0.5.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.3.0 to 0.4.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.3.0...noosphere-v0.4.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.3.0 to 0.4.0
    * noosphere-fs bumped from 0.2.0 to 0.3.0
    * noosphere-storage bumped from 0.2.0 to 0.3.0
    * noosphere-api bumped from 0.4.0 to 0.5.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.2.0...noosphere-v0.3.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`

### Features

* Introduce pet names to spheres ([#154](https://github.com/subconsciousnetwork/noosphere/issues/154)) ([7495796](https://github.com/subconsciousnetwork/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/subconsciousnetwork/noosphere/issues/162)) ([1e83bbb](https://github.com/subconsciousnetwork/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/subconsciousnetwork/noosphere/issues/177)) ([e269e04](https://github.com/subconsciousnetwork/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.2.0 to 0.3.0
    * noosphere-fs bumped from 0.1.0 to 0.2.0
    * noosphere-storage bumped from 0.1.0 to 0.2.0
    * noosphere-api bumped from 0.3.0 to 0.4.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0...noosphere-v0.2.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/subconsciousnetwork/noosphere/issues/141)) ([26e34ac](https://github.com/subconsciousnetwork/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0
    * noosphere-fs bumped from 0.1.1-alpha.1 to 0.1.0
    * noosphere-api bumped from 0.2.0 to 0.3.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0-alpha.1...noosphere-v0.1.0) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0 to 0.2.0

## [0.1.0-alpha.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0-alpha.1...noosphere-v0.1.0-alpha.1) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0-alpha.1 to 0.1.0
    * noosphere-fs bumped from 0.1.0-alpha.1 to 0.1.1-alpha.1
    * noosphere-storage bumped from 0.1.0-alpha.1 to 0.1.0

## 0.1.0-alpha.1 (2022-11-03)


### Features

* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/subconsciousnetwork/noosphere/issues/123)) ([ad9daa6](https://github.com/subconsciousnetwork/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0-alpha.1 to 0.1.0
