# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.0 to 0.4.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.2 to 0.5.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.0 to 0.6.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.2 to 0.6.3

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.0 to 0.7.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.1 to 0.8.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.8.0 to 0.8.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.8.1 to 0.9.0

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.5.3...noosphere-collections-v0.6.0) (2023-06-08)


### ⚠ BREAKING CHANGES

* Migrate blake2b->blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400))

### Features

* Consolidate `NsRecord` implementation in`LinkRecord`. Fixes [#395](https://github.com/subconsciousnetwork/noosphere/issues/395) ([#399](https://github.com/subconsciousnetwork/noosphere/issues/399)) ([9ee4798](https://github.com/subconsciousnetwork/noosphere/commit/9ee47981232fde00b34bb9458c5b0b2799a610ca))
* Migrate blake2b-&gt;blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400)) ([f9e0aec](https://github.com/subconsciousnetwork/noosphere/commit/f9e0aecd76a7253aba13b1881af32a2e543fb6de)), closes [#386](https://github.com/subconsciousnetwork/noosphere/issues/386)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.3 to 0.7.0

## [0.5.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.5.1...noosphere-collections-v0.5.2) (2023-04-22)


### Features

* Update IPLD-related dependencies ([#327](https://github.com/subconsciousnetwork/noosphere/issues/327)) ([5fdfadb](https://github.com/subconsciousnetwork/noosphere/commit/5fdfadb1656f9d6eef2dbbb8b00a598106bccf00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.1 to 0.6.2

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.4.0...noosphere-collections-v0.5.0) (2023-04-04)


### ⚠ BREAKING CHANGES

* Apply breaking domain concept in anticipation of beta (#298)

### Miscellaneous Chores

* Apply breaking domain concept in anticipation of beta ([#298](https://github.com/subconsciousnetwork/noosphere/issues/298)) ([bd34ab4](https://github.com/subconsciousnetwork/noosphere/commit/bd34ab49b2d2c65cffe25657cf4d188d5c79d15f))

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.3.3...noosphere-collections-v0.4.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Traverse the Noosphere vast (#284)

### Features

* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.5.0 to 0.6.0

## [0.3.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.3.1...noosphere-collections-v0.3.2) (2023-02-16)


### Features

* Always flush on SphereFS save ([#231](https://github.com/subconsciousnetwork/noosphere/issues/231)) ([bd151d5](https://github.com/subconsciousnetwork/noosphere/commit/bd151d5aca75b78b786d008177ab7d4e53e843bc))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.1 to 0.4.2

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.2.0...noosphere-collections-v0.3.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.3.0 to 0.4.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.1.0...noosphere-collections-v0.2.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.2.0 to 0.3.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.1.1-alpha.1...noosphere-collections-v0.1.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.

### Features

* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0 to 0.2.0
