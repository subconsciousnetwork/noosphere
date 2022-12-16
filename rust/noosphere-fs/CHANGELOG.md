# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.0 to 0.6.1

## [0.5.0](https://github.com/cdata/noosphere/compare/noosphere-fs-v0.4.0...noosphere-fs-v0.5.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.5.0 to 0.6.0
    * noosphere-storage bumped from 0.4.0 to 0.5.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-fs-v0.3.0...noosphere-fs-v0.4.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.4.0 to 0.5.0
    * noosphere-storage bumped from 0.3.0 to 0.4.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-fs-v0.2.0...noosphere-fs-v0.3.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.3.0 to 0.4.0
    * noosphere-storage bumped from 0.2.0 to 0.3.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-fs-v0.1.0...noosphere-fs-v0.2.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* `SphereIpld` identity is now a `Did`

### Features

* Introduce pet names to spheres ([#154](https://github.com/subconsciousnetwork/noosphere/issues/154)) ([7495796](https://github.com/subconsciousnetwork/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.2.0 to 0.3.0
    * noosphere-storage bumped from 0.1.0 to 0.2.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-fs-v0.1.1-alpha.1...noosphere-fs-v0.1.0) (2022-11-14)


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
