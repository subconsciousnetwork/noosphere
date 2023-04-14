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

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.5.0 to 0.6.0
    * noosphere-core bumped from 0.7.0 to 0.8.0
    * noosphere bumped from 0.7.0 to 0.8.0
    * noosphere-ipfs bumped from 0.2.0 to 0.3.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.8.0 to 0.9.0
    * noosphere bumped from 0.8.0 to 0.8.1
    * noosphere-ipfs bumped from 0.3.0 to 0.3.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere bumped from 0.8.3 to 0.8.4

## [0.5.4](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.5.3...noosphere-ns-v0.5.4) (2023-04-13)


### Bug Fixes

* Increase timeout in DHT network tests to satisfy CI, fixes [#311](https://github.com/subconsciousnetwork/noosphere/issues/311) ([#312](https://github.com/subconsciousnetwork/noosphere/issues/312)) ([2f9f1a6](https://github.com/subconsciousnetwork/noosphere/commit/2f9f1a6bbcc394672dfd2b93e4b1255f0fa9529b))
* Intermittent timeouts in DhtNode tests introduced in [#308](https://github.com/subconsciousnetwork/noosphere/issues/308) ([#316](https://github.com/subconsciousnetwork/noosphere/issues/316)) ([704652b](https://github.com/subconsciousnetwork/noosphere/commit/704652bba2a2d9b241799b97808c7a249f0c38a9))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere bumped from 0.8.2 to 0.8.3

## [0.5.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.5.2...noosphere-ns-v0.5.3) (2023-04-10)


### Features

* Add instrumentation to `noosphere-ns` and `noosphere-ipfs`. ([#304](https://github.com/subconsciousnetwork/noosphere/issues/304)) ([3d6062d](https://github.com/subconsciousnetwork/noosphere/commit/3d6062d501e21393532b2db6f9ac740a041d91ba))
* cache 'peer_id' in orb-ns to provide a HTTP route that does not lock the NS mutex for testing. ([#303](https://github.com/subconsciousnetwork/noosphere/issues/303)) ([8e4769f](https://github.com/subconsciousnetwork/noosphere/commit/8e4769f548b486147a9b1e72d86555fe4246fa14))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.0 to 0.6.1
    * noosphere-core bumped from 0.9.0 to 0.9.1
    * noosphere bumped from 0.8.1 to 0.8.2
    * noosphere-ipfs bumped from 0.3.1 to 0.3.2

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.4.3...noosphere-ns-v0.5.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Petname resolution and synchronization in spheres and gateways (#253)
* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. (#252)

### Features

* Expose ipfs-api-url to orb-ns to integrate IPFS cid resolution in NS validation. ([#265](https://github.com/subconsciousnetwork/noosphere/issues/265)) ([d1bdc29](https://github.com/subconsciousnetwork/noosphere/commit/d1bdc29d28dc28e99eca794c11b4d190b7128dfe))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Bug Fixes

* Limit delegated UCAN's lifetime to authorization token's lifetime where appropriate. ([#249](https://github.com/subconsciousnetwork/noosphere/issues/249)) ([b62fb88](https://github.com/subconsciousnetwork/noosphere/commit/b62fb888e16718cb84f33aa93c14385ddef4d8d1))


### Miscellaneous Chores

* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. ([#252](https://github.com/subconsciousnetwork/noosphere/issues/252)) ([518beae](https://github.com/subconsciousnetwork/noosphere/commit/518beae563bd04c921ee3c6641a7249f14c611e4))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.2 to 0.5.0
    * noosphere-core bumped from 0.6.3 to 0.7.0
    * noosphere bumped from 0.6.3 to 0.7.0
    * noosphere-ipfs bumped from 0.1.2 to 0.2.0

## [0.4.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ns-v0.4.2...noosphere-ns-v0.4.3) (2023-02-16)


### Features

* Follow up of initial orb-ns implementation. ([#222](https://github.com/subconsciousnetwork/noosphere/issues/222)) ([bb4c53f](https://github.com/subconsciousnetwork/noosphere/commit/bb4c53f3e79de6f5f66cc5b83ec815864f6bc5ab))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.1 to 0.4.2
    * noosphere-core bumped from 0.6.2 to 0.6.3
    * noosphere bumped from 0.6.2 to 0.6.3

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
