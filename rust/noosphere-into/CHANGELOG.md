# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.5.0 to 0.5.1
    * noosphere-storage bumped from 0.4.0 to 0.4.1
    * noosphere-fs bumped from 0.4.0 to 0.4.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.5.1 to 0.6.0
    * noosphere-fs bumped from 0.4.1 to 0.5.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.0 to 0.6.1
    * noosphere-fs bumped from 0.5.0 to 0.5.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.1 to 0.6.2
    * noosphere-fs bumped from 0.5.1 to 0.5.2

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.2 to 0.6.3
    * noosphere-storage bumped from 0.4.1 to 0.4.2
    * noosphere-fs bumped from 0.5.2 to 0.5.3

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.5.0...noosphere-into-v0.6.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Traverse the Noosphere vast (#284)
* Revise links and gateway (#278)

### Features

* Revise links and gateway ([#278](https://github.com/subconsciousnetwork/noosphere/issues/278)) ([4cd2e3a](https://github.com/subconsciousnetwork/noosphere/commit/4cd2e3af8b10cdaae710d87e4b919b5180d10fec))
* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.7.0 to 0.8.0
    * noosphere-storage bumped from 0.5.0 to 0.6.0
    * noosphere-sphere bumped from 0.1.0 to 0.2.0
  * dev-dependencies
    * noosphere-sphere bumped from 0.1.0 to 0.2.0

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.4.5...noosphere-into-v0.5.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Petname resolution and synchronization in spheres and gateways (#253)

### Features

* Noosphere builds and runs tests on Windows ([#228](https://github.com/subconsciousnetwork/noosphere/issues/228)) ([d1320f0](https://github.com/subconsciousnetwork/noosphere/commit/d1320f08429c8f8090fd4612b56ebf9386414cc7))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.3 to 0.7.0
    * noosphere-storage bumped from 0.4.2 to 0.5.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.3.0...noosphere-into-v0.4.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.4.0 to 0.5.0
    * noosphere-storage bumped from 0.3.0 to 0.4.0
    * noosphere-fs bumped from 0.3.0 to 0.4.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.2.0...noosphere-into-v0.3.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.3.0 to 0.4.0
    * noosphere-storage bumped from 0.2.0 to 0.3.0
    * noosphere-fs bumped from 0.2.0 to 0.3.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.1.0...noosphere-into-v0.2.0) (2022-11-29)


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
    * noosphere-fs bumped from 0.1.0 to 0.2.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-into-v0.1.1-alpha.1...noosphere-into-v0.1.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0
    * noosphere-fs bumped from 0.1.1-alpha.1 to 0.1.0
