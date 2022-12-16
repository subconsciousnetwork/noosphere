# Changelog

## [0.5.0](https://github.com/cdata/noosphere/compare/noosphere-collections-v0.4.0...noosphere-collections-v0.5.0) (2022-12-16)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.4.0](https://github.com/cdata/noosphere/compare/noosphere-collections-v0.3.0...noosphere-collections-v0.4.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.0 to 0.5.0

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
