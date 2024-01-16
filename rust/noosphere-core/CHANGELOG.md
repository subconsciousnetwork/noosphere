# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.1 to 0.4.2
    * noosphere-collections bumped from 0.3.1 to 0.3.2

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.0 to 0.7.1
    * noosphere-collections bumped from 0.6.0 to 0.6.1

## [0.18.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.17.3...noosphere-core-v0.18.0) (2023-11-21)


### ⚠ BREAKING CHANGES

* Initial work refactoring noosphere-gateway to be generic over the ([#698](https://github.com/subconsciousnetwork/noosphere/issues/698))

### Features

* Initial work refactoring noosphere-gateway to be generic over the ([#698](https://github.com/subconsciousnetwork/noosphere/issues/698)) ([92f0d8a](https://github.com/subconsciousnetwork/noosphere/commit/92f0d8a6ff4a76dd971f6e945fcc7ddb01019699))
* Rewrite host headers in API client to embed sphere identity in subdomain when feature 'test-gateway' is enabled. ([#726](https://github.com/subconsciousnetwork/noosphere/issues/726)) ([e55f5f1](https://github.com/subconsciousnetwork/noosphere/commit/e55f5f180c3f6f6ad9a8a35a3387830f80957938))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.9.3 to 0.10.0
    * noosphere-collections bumped from 0.6.7 to 0.7.0

## [0.17.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.17.2...noosphere-core-v0.17.3) (2023-11-01)


### Features

* Periodic syndication checks to IPFS Kubo ([#685](https://github.com/subconsciousnetwork/noosphere/issues/685)) ([b5640b2](https://github.com/subconsciousnetwork/noosphere/commit/b5640b2e23ad7bfc522a03d0b1731e372425afa8))


### Bug Fixes

* Recovery only uses latest version of sphere ([#703](https://github.com/subconsciousnetwork/noosphere/issues/703)) ([500bd69](https://github.com/subconsciousnetwork/noosphere/commit/500bd69509b21e7f8c13b178f1de05168b2386d3))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.9.2 to 0.9.3
    * noosphere-collections bumped from 0.6.6 to 0.6.7

## [0.17.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.17.1...noosphere-core-v0.17.2) (2023-10-12)


### Features

* 3P replication fall-back and resilience ([#673](https://github.com/subconsciousnetwork/noosphere/issues/673)) ([08dcc3d](https://github.com/subconsciousnetwork/noosphere/commit/08dcc3d54768fdda6158b1087a32805a5c855e98))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-common bumped from 0.1.1 to 0.1.2
    * noosphere-storage bumped from 0.9.1 to 0.9.2
    * noosphere-collections bumped from 0.6.5 to 0.6.6
  * dev-dependencies
    * noosphere-common bumped from 0.1.1 to 0.1.2

## [0.17.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.17.0...noosphere-core-v0.17.1) (2023-10-06)


### Features

* Improved IPFS Kubo syndication ([#666](https://github.com/subconsciousnetwork/noosphere/issues/666)) ([eeab932](https://github.com/subconsciousnetwork/noosphere/commit/eeab932763cd642702bc6ac85a6bbc10968a107d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-common bumped from 0.1.0 to 0.1.1
    * noosphere-storage bumped from 0.9.0 to 0.9.1
    * noosphere-collections bumped from 0.6.4 to 0.6.5
  * dev-dependencies
    * noosphere-common bumped from 0.1.0 to 0.1.1

## [0.17.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.16.0...noosphere-core-v0.17.0) (2023-10-04)


### ⚠ BREAKING CHANGES

* History compaction for spheres and gateways ([#661](https://github.com/subconsciousnetwork/noosphere/issues/661))

### Features

* History compaction for spheres and gateways ([#661](https://github.com/subconsciousnetwork/noosphere/issues/661)) ([b8f41b6](https://github.com/subconsciousnetwork/noosphere/commit/b8f41b6bea9393e0dddfce94fd2fd5e4188e28d7))

## [0.16.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.15.2...noosphere-core-v0.16.0) (2023-09-19)


### ⚠ BREAKING CHANGES

* Disaster recovery via gateway ([#637](https://github.com/subconsciousnetwork/noosphere/issues/637))
* Replace `Bundle` with CAR streams in push ([#624](https://github.com/subconsciousnetwork/noosphere/issues/624))

### Features

* Disaster recovery via gateway ([#637](https://github.com/subconsciousnetwork/noosphere/issues/637)) ([70e7331](https://github.com/subconsciousnetwork/noosphere/commit/70e7331767f65e0976ee5843229f765dc6ace7fb))
* Introduce RocksDbStorage, genericize storage throughout. ([#623](https://github.com/subconsciousnetwork/noosphere/issues/623)) ([7155f86](https://github.com/subconsciousnetwork/noosphere/commit/7155f860c2f5ee481d923941dca95981cd7f4b38))
* Replace `Bundle` with CAR streams in push ([#624](https://github.com/subconsciousnetwork/noosphere/issues/624)) ([9390797](https://github.com/subconsciousnetwork/noosphere/commit/9390797eb6653fdecd41c3a54225ffd55945bb89))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.8.1 to 0.9.0
    * noosphere-collections bumped from 0.6.3 to 0.6.4

## [0.15.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.15.1...noosphere-core-v0.15.2) (2023-08-29)


### Features

* Ensure adopted link records are fresher than previous entries. Fixes [#258](https://github.com/subconsciousnetwork/noosphere/issues/258), fixes [#562](https://github.com/subconsciousnetwork/noosphere/issues/562) ([#578](https://github.com/subconsciousnetwork/noosphere/issues/578)) ([36e42fb](https://github.com/subconsciousnetwork/noosphere/commit/36e42fb03424858e7731d10ad0a0cf89826b1354))

## [0.15.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.15.0...noosphere-core-v0.15.1) (2023-08-10)


### Features

* `orb sphere history` and `orb sphere render` ([#576](https://github.com/subconsciousnetwork/noosphere/issues/576)) ([a6f0a74](https://github.com/subconsciousnetwork/noosphere/commit/a6f0a74cde2fc001bfff5c1bed0844ac19fc8258))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.8.0 to 0.8.1
    * noosphere-collections bumped from 0.6.2 to 0.6.3

## [0.15.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.14.0...noosphere-core-v0.15.0) (2023-08-04)


### ⚠ BREAKING CHANGES

* `orb` uses latest Noosphere capabilities ([#530](https://github.com/subconsciousnetwork/noosphere/issues/530))

### Features

* `orb` uses latest Noosphere capabilities ([#530](https://github.com/subconsciousnetwork/noosphere/issues/530)) ([adfa028](https://github.com/subconsciousnetwork/noosphere/commit/adfa028ebcb2de7ea7492af57239fcc9bfc27955))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.1 to 0.8.0
    * noosphere-collections bumped from 0.6.1 to 0.6.2

## [0.14.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.13.2...noosphere-core-v0.14.0) (2023-07-20)


### ⚠ BREAKING CHANGES

* C FFI to verify authorizations ([#510](https://github.com/subconsciousnetwork/noosphere/issues/510))

### Features

* C FFI to verify authorizations ([#510](https://github.com/subconsciousnetwork/noosphere/issues/510)) ([ed092fc](https://github.com/subconsciousnetwork/noosphere/commit/ed092fc303f89ca4737f5e67681e2ede8189304d))

## [0.13.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.13.1...noosphere-core-v0.13.2) (2023-07-19)


### Features

* Introduce structured log format ([#484](https://github.com/subconsciousnetwork/noosphere/issues/484)) ([30820f4](https://github.com/subconsciousnetwork/noosphere/commit/30820f425cef90d48687a4e4fd09552a28a1936e))

## [0.13.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.12.3...noosphere-core-v0.13.0) (2023-07-01)


### ⚠ BREAKING CHANGES

* Authorize and revoke APIs ([#420](https://github.com/subconsciousnetwork/noosphere/issues/420))
* Update to `rs-ucan` 0.4.0, implementing UCAN 0.10ish. ([#449](https://github.com/subconsciousnetwork/noosphere/issues/449))

### Features

* Authorize and revoke APIs ([#420](https://github.com/subconsciousnetwork/noosphere/issues/420)) ([73f016e](https://github.com/subconsciousnetwork/noosphere/commit/73f016e12448c46f95ae7683d91fd6422a925555))
* Update to `rs-ucan` 0.4.0, implementing UCAN 0.10ish. ([#449](https://github.com/subconsciousnetwork/noosphere/issues/449)) ([8b806c5](https://github.com/subconsciousnetwork/noosphere/commit/8b806c5462b5601a5f8417a6a20769b76b57ee6a))

## [0.12.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.12.2...noosphere-core-v0.12.3) (2023-06-23)


### Bug Fixes

* Remove ineffective Sentry initialization ([200154c](https://github.com/subconsciousnetwork/noosphere/commit/200154cc82c0915a912df455c2e8cb8c619612ac))

## [0.12.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.12.1...noosphere-core-v0.12.2) (2023-06-22)


### Features

* Add support for sentry in `noosphere-core` tracing ([#437](https://github.com/subconsciousnetwork/noosphere/issues/437)) ([3b71541](https://github.com/subconsciousnetwork/noosphere/commit/3b7154150167f5964776c888470662305caefd18))

## [0.12.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.12.0...noosphere-core-v0.12.1) (2023-06-09)


### Bug Fixes

* Resolve petnames in correct order ([#412](https://github.com/subconsciousnetwork/noosphere/issues/412)) ([5df3f91](https://github.com/subconsciousnetwork/noosphere/commit/5df3f9187be1d0ef6edd542a9d1268c7cb4ffdb7))

## [0.12.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.11.0...noosphere-core-v0.12.0) (2023-06-08)


### ⚠ BREAKING CHANGES

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409))

### Features

* Consolidate `NsRecord` implementation in`LinkRecord`. Fixes [#395](https://github.com/subconsciousnetwork/noosphere/issues/395) ([#399](https://github.com/subconsciousnetwork/noosphere/issues/399)) ([9ee4798](https://github.com/subconsciousnetwork/noosphere/commit/9ee47981232fde00b34bb9458c5b0b2799a610ca))


### Bug Fixes

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409)) ([8812a1e](https://github.com/subconsciousnetwork/noosphere/commit/8812a1e8c9348301b36b77d6c1a2024432806358))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.3 to 0.7.0
    * noosphere-collections bumped from 0.5.3 to 0.6.0

## [0.11.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.10.2...noosphere-core-v0.11.0) (2023-05-08)


### ⚠ BREAKING CHANGES

* Revised tracing configuration ([#342](https://github.com/subconsciousnetwork/noosphere/issues/342))

### Features

* Enable expired yet valid records in the name system. Update to ucan 0.2.0. ([#360](https://github.com/subconsciousnetwork/noosphere/issues/360)) ([3b0663a](https://github.com/subconsciousnetwork/noosphere/commit/3b0663abc7783a6d33dd47d20caae7597ab93ed0))
* Revised tracing configuration ([#342](https://github.com/subconsciousnetwork/noosphere/issues/342)) ([c4a4084](https://github.com/subconsciousnetwork/noosphere/commit/c4a4084771680c8e49b3db498a5da422db2adda8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.2 to 0.6.3
    * noosphere-collections bumped from 0.5.2 to 0.5.3

## [0.10.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.10.1...noosphere-core-v0.10.2) (2023-05-05)


### Features

* Enable expired yet valid records in the name system. Update to ucan 0.2.0. ([#360](https://github.com/subconsciousnetwork/noosphere/issues/360)) ([3b0663a](https://github.com/subconsciousnetwork/noosphere/commit/3b0663abc7783a6d33dd47d20caae7597ab93ed0))
* Publish name record from gateway periodically. ([#334](https://github.com/subconsciousnetwork/noosphere/issues/334)) ([fc5e42f](https://github.com/subconsciousnetwork/noosphere/commit/fc5e42f2bd918fc1b3c448e55c611a99d49b00db))
* Remove `Mutex` from NNS `ApiServer` for concurrency ([#357](https://github.com/subconsciousnetwork/noosphere/issues/357)) ([2347d10](https://github.com/subconsciousnetwork/noosphere/commit/2347d10490fbb7ecc219a3a09c1de21e11f66fa2))

## [0.10.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.10.0...noosphere-core-v0.10.1) (2023-05-02)


### Bug Fixes

* Remove vestigial `tracing-core` dependency ([#348](https://github.com/subconsciousnetwork/noosphere/issues/348)) ([31528c6](https://github.com/subconsciousnetwork/noosphere/commit/31528c6083190b5298b90b9a8af7f4eff3836b99))

## [0.10.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.9.3...noosphere-core-v0.10.0) (2023-05-02)


### ⚠ BREAKING CHANGES

* Revised tracing configuration (#342)

### Features

* Revised tracing configuration ([#342](https://github.com/subconsciousnetwork/noosphere/issues/342)) ([c4a4084](https://github.com/subconsciousnetwork/noosphere/commit/c4a4084771680c8e49b3db498a5da422db2adda8))

## [0.9.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.9.2...noosphere-core-v0.9.3) (2023-04-22)


### Features

* Update IPLD-related dependencies ([#327](https://github.com/subconsciousnetwork/noosphere/issues/327)) ([5fdfadb](https://github.com/subconsciousnetwork/noosphere/commit/5fdfadb1656f9d6eef2dbbb8b00a598106bccf00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.1 to 0.6.2
    * noosphere-collections bumped from 0.5.1 to 0.5.2

## [0.9.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.9.1...noosphere-core-v0.9.2) (2023-04-19)


### Features

* Sphere writes do not block immutable reads ([#321](https://github.com/subconsciousnetwork/noosphere/issues/321)) ([14373c5](https://github.com/subconsciousnetwork/noosphere/commit/14373c5281c091bb41623677571566a2788a7e3f))

## [0.9.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.9.0...noosphere-core-v0.9.1) (2023-04-10)


### Features

* Dot syntax when traversing by petname ([#306](https://github.com/subconsciousnetwork/noosphere/issues/306)) ([cd87b05](https://github.com/subconsciousnetwork/noosphere/commit/cd87b0533c21bbbd4d82332556e70ecc706a5531))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.0 to 0.6.1
    * noosphere-collections bumped from 0.5.0 to 0.5.1

## [0.9.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.8.0...noosphere-core-v0.9.0) (2023-04-04)


### ⚠ BREAKING CHANGES

* Apply breaking domain concept in anticipation of beta (#298)

### Features

* Introduce `Link`, a typed `Cid` ([#297](https://github.com/subconsciousnetwork/noosphere/issues/297)) ([9520826](https://github.com/subconsciousnetwork/noosphere/commit/9520826029235e5dc32adca77193b4f82b9de80c))


### Miscellaneous Chores

* Apply breaking domain concept in anticipation of beta ([#298](https://github.com/subconsciousnetwork/noosphere/issues/298)) ([bd34ab4](https://github.com/subconsciousnetwork/noosphere/commit/bd34ab49b2d2c65cffe25657cf4d188d5c79d15f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-collections bumped from 0.4.0 to 0.5.0

## [0.8.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.7.0...noosphere-core-v0.8.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Traverse the Noosphere vast (#284)
* Revise links and gateway (#278)

### Features

* Revise links and gateway ([#278](https://github.com/subconsciousnetwork/noosphere/issues/278)) ([4cd2e3a](https://github.com/subconsciousnetwork/noosphere/commit/4cd2e3af8b10cdaae710d87e4b919b5180d10fec))
* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.5.0 to 0.6.0
    * noosphere-collections bumped from 0.3.3 to 0.4.0

## [0.7.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.6.3...noosphere-core-v0.7.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Petname resolution and synchronization in spheres and gateways (#253)

### Features

* Introduce `noosphere-gateway` crate ([#238](https://github.com/subconsciousnetwork/noosphere/issues/238)) ([791bc39](https://github.com/subconsciousnetwork/noosphere/commit/791bc3996cfac12cb077c3721f22d080a71d33ba))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Bug Fixes

* Limit delegated UCAN's lifetime to authorization token's lifetime where appropriate. ([#249](https://github.com/subconsciousnetwork/noosphere/issues/249)) ([b62fb88](https://github.com/subconsciousnetwork/noosphere/commit/b62fb888e16718cb84f33aa93c14385ddef4d8d1))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.2 to 0.5.0
    * noosphere-collections bumped from 0.3.2 to 0.3.3

## [0.6.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.6.1...noosphere-core-v0.6.2) (2023-02-07)


### Features

* General error handling in C FFI ([#219](https://github.com/subconsciousnetwork/noosphere/issues/219)) ([0a1952b](https://github.com/subconsciousnetwork/noosphere/commit/0a1952b34895071d2203505c95750d453bb110c6))

## [0.6.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.6.0...noosphere-core-v0.6.1) (2023-02-02)


### Bug Fixes

* Ensure that sphere changes exclude `since` ([#216](https://github.com/subconsciousnetwork/noosphere/issues/216)) ([31fee07](https://github.com/subconsciousnetwork/noosphere/commit/31fee07424a019db21773947a5fe5a17a80f1c45))

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.5.1...noosphere-core-v0.6.0) (2023-01-31)


### ⚠ BREAKING CHANGES

* Sphere sync and change diff in C FFI (#210)

### Features

* Sphere sync and change diff in C FFI ([#210](https://github.com/subconsciousnetwork/noosphere/issues/210)) ([306d39c](https://github.com/subconsciousnetwork/noosphere/commit/306d39cdf6727fbeb34a49740b55f56834f4df07))

## [0.5.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.5.0...noosphere-core-v0.5.1) (2023-01-19)


### Features

* Improvements to the NameSystem based on initial gateway integration ([#196](https://github.com/subconsciousnetwork/noosphere/issues/196)) ([4a6898e](https://github.com/subconsciousnetwork/noosphere/commit/4a6898e0aa8e1d96780226d384a6876eac122658))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.0 to 0.4.1
    * noosphere-collections bumped from 0.3.0 to 0.3.1

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.4.0...noosphere-core-v0.5.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.3.0 to 0.4.0
    * noosphere-collections bumped from 0.2.0 to 0.3.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.3.0...noosphere-core-v0.4.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.2.0 to 0.3.0
    * noosphere-collections bumped from 0.1.0 to 0.2.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.2.0...noosphere-core-v0.3.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`

### Features

* Introduce pet names to spheres ([#154](https://github.com/subconsciousnetwork/noosphere/issues/154)) ([7495796](https://github.com/subconsciousnetwork/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Mutation and hydration for names ([#168](https://github.com/subconsciousnetwork/noosphere/issues/168)) ([5e2a1ca](https://github.com/subconsciousnetwork/noosphere/commit/5e2a1ca369875c425c0612c4ac7df0a942f8fcab))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/subconsciousnetwork/noosphere/issues/162)) ([1e83bbb](https://github.com/subconsciousnetwork/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/subconsciousnetwork/noosphere/issues/177)) ([e269e04](https://github.com/subconsciousnetwork/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0 to 0.2.0
    * noosphere-collections bumped from 0.1.1-alpha.1 to 0.1.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.1.0...noosphere-core-v0.2.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-core-v0.1.0-alpha.1...noosphere-core-v0.1.0) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.
* initial work on NameSystem, wrapping the underlying DHT network. (#122)

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* initial work on NameSystem, wrapping the underlying DHT network. ([#122](https://github.com/subconsciousnetwork/noosphere/issues/122)) ([656fb23](https://github.com/subconsciousnetwork/noosphere/commit/656fb23a5ce5a75b7f1de59444c1d866a9308d83))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.1.0-alpha.1 to 0.1.0
    * noosphere-collections bumped from 0.1.0-alpha.1 to 0.1.1-alpha.1
