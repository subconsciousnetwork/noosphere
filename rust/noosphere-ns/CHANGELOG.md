# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.3.0 to 0.4.0
    * noosphere-core bumped from 0.4.0 to 0.5.0
    * noosphere bumped from 0.4.0 to 0.5.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.0 to 0.6.1
    * noosphere bumped from 0.6.0 to 0.6.1

## [0.4.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.4.1...noosphere-ns-v0.4.2) (2023-02-07)


### Features

* Integration of orb-ns CLI into the Name System's operator API ([#218](https://github.com/subconsciousnetwork/noosphere/issues/218)) ([7f83fad](https://github.com/subconsciousnetwork/noosphere/commit/7f83fad1f318ec45eb47de76ca855f9eab4fe688))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.1 to 0.6.2
    * noosphere bumped from 0.6.1 to 0.6.2

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.3.2...noosphere-ns-v0.4.0) (2023-01-31)


### ⚠ BREAKING CHANGES

* upgrade libp2p to 0.50.0 (#209)

### Features

* DHT configuration and status API ([#207](https://github.com/subconsciousnetwork/noosphere/issues/207)) ([7e671cf](https://github.com/subconsciousnetwork/noosphere/commit/7e671cfe06768e7faadd9d2573a11c899ae9cb22))
* upgrade libp2p to 0.50.0 ([#209](https://github.com/subconsciousnetwork/noosphere/issues/209)) ([14ab195](https://github.com/subconsciousnetwork/noosphere/commit/14ab195b797bcb23d1ed25a8eacc3fc37e30c0ce))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.5.1 to 0.6.0
    * noosphere bumped from 0.5.1 to 0.6.0

## [0.3.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.3.1...noosphere-ns-v0.3.2) (2023-01-19)


### Features

* Improvements to the NameSystem based on initial gateway integration ([#196](https://github.com/subconsciousnetwork/noosphere/issues/196)) ([4a6898e](https://github.com/subconsciousnetwork/noosphere/commit/4a6898e0aa8e1d96780226d384a6876eac122658))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.0 to 0.4.1
    * noosphere-core bumped from 0.5.0 to 0.5.1
    * noosphere bumped from 0.5.0 to 0.5.1

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.2.0...noosphere-ns-v0.3.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.2.0 to 0.3.0
    * noosphere-core bumped from 0.3.0 to 0.4.0
    * noosphere bumped from 0.3.0 to 0.4.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.1.1...noosphere-ns-v0.2.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.

### Features

* Introduce a `bootstrap` CLI in `noosphere-ns` to spin up DHT ([#143](https://github.com/subconsciousnetwork/noosphere/issues/143)) ([c5f2710](https://github.com/subconsciousnetwork/noosphere/commit/c5f27103cf6b8f597da0a3707fed45a494023920))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/subconsciousnetwork/noosphere/issues/162)) ([1e83bbb](https://github.com/subconsciousnetwork/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0 to 0.2.0
    * noosphere-core bumped from 0.2.0 to 0.3.0
    * noosphere bumped from 0.2.0 to 0.3.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.1.0-alpha.1...noosphere-ns-v0.1.0) (2022-11-09)


### ⚠ BREAKING CHANGES

* initial work on NameSystem, wrapping the underlying DHT network. (#122)

### Features

* Expose replication/publication/ttl intervals to NameSystemBuilder ([#130](https://github.com/subconsciousnetwork/noosphere/issues/130)) ([e20680e](https://github.com/subconsciousnetwork/noosphere/commit/e20680e225d53d8c658a9c6c2ba5dcb80d2a314e))
* Implement a RecordValidator trait for the NameSystem DHT ([#129](https://github.com/subconsciousnetwork/noosphere/issues/129)) ([ba5560c](https://github.com/subconsciousnetwork/noosphere/commit/ba5560c031f2251a984eeaa0e0a7c95ad63e3c70))
* initial work on NameSystem, wrapping the underlying DHT network. ([#122](https://github.com/subconsciousnetwork/noosphere/issues/122)) ([656fb23](https://github.com/subconsciousnetwork/noosphere/commit/656fb23a5ce5a75b7f1de59444c1d866a9308d83))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0-alpha.1 to 0.1.0
    * noosphere-core bumped from 0.1.0-alpha.1 to 0.1.0
