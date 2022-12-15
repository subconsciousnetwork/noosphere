# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0-alpha.1 to 0.1.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0 to 0.2.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere bumped from 0.7.0 to 0.8.0

## [0.8.0](https://github.com/cdata/noosphere/compare/noosphere-cli-v0.7.0...noosphere-cli-v0.8.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* **cli:** Find the nearest ancestor sphere ([#119](https://github.com/cdata/noosphere/issues/119)) ([9e33026](https://github.com/cdata/noosphere/commit/9e3302623db3af88df626ccb02ad8fa699e79223))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Bug Fixes

* Recover from Kubo pin check ([#193](https://github.com/cdata/noosphere/issues/193)) ([b0e0851](https://github.com/cdata/noosphere/commit/b0e0851a5748c88c05977091abd780cf1a4f12ce))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere bumped from 0.6.0 to 0.7.0

## [0.7.0](https://github.com/cdata/noosphere/compare/noosphere-cli-v0.6.0...noosphere-cli-v0.7.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* **cli:** Find the nearest ancestor sphere ([#119](https://github.com/cdata/noosphere/issues/119)) ([9e33026](https://github.com/cdata/noosphere/commit/9e3302623db3af88df626ccb02ad8fa699e79223))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Bug Fixes

* Recover from Kubo pin check ([#193](https://github.com/cdata/noosphere/issues/193)) ([b0e0851](https://github.com/cdata/noosphere/commit/b0e0851a5748c88c05977091abd780cf1a4f12ce))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.5.0 to 0.6.0
    * noosphere-fs bumped from 0.4.0 to 0.5.0
    * noosphere-storage bumped from 0.4.0 to 0.5.0
    * noosphere-api bumped from 0.5.1 to 0.6.0
    * noosphere bumped from 0.5.0 to 0.6.0

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.5.0...noosphere-cli-v0.6.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))


### Bug Fixes

* Recover from Kubo pin check ([#193](https://github.com/subconsciousnetwork/noosphere/issues/193)) ([b0e0851](https://github.com/subconsciousnetwork/noosphere/commit/b0e0851a5748c88c05977091abd780cf1a4f12ce))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.4.0 to 0.5.0
    * noosphere-fs bumped from 0.3.0 to 0.4.0
    * noosphere-storage bumped from 0.3.0 to 0.4.0
    * noosphere-api bumped from 0.5.0 to 0.5.1
    * noosphere bumped from 0.4.0 to 0.5.0

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.4.0...noosphere-cli-v0.5.0) (2022-11-30)


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
    * noosphere bumped from 0.3.0 to 0.4.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.3.0...noosphere-cli-v0.4.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.

### Features

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
    * noosphere bumped from 0.2.0 to 0.3.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.2.1...noosphere-cli-v0.3.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0
    * noosphere-fs bumped from 0.1.1-alpha.1 to 0.1.0
    * noosphere-api bumped from 0.2.0 to 0.3.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.1.1...noosphere-cli-v0.2.0) (2022-11-09)


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

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.1.0-alpha.1...noosphere-cli-v0.1.0) (2022-10-31)


### Features

* **cli:** Find the nearest ancestor sphere ([#119](https://github.com/subconsciousnetwork/noosphere/issues/119)) ([9e33026](https://github.com/subconsciousnetwork/noosphere/commit/9e3302623db3af88df626ccb02ad8fa699e79223))
