# Changelog

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-collections-v0.1.1-alpha.1...noosphere-collections-v0.1.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.

### Features

* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0 to 0.2.0
