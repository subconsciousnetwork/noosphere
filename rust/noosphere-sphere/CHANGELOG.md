# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.10.0 to 0.10.1
    * noosphere-api bumped from 0.7.5 to 0.7.6
    * noosphere-ipfs bumped from 0.4.0 to 0.4.1

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.10.1 to 0.10.2
    * noosphere-api bumped from 0.7.6 to 0.7.7
    * noosphere-ipfs bumped from 0.4.1 to 0.4.2

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.7.8 to 0.7.9
    * noosphere-ipfs bumped from 0.4.3 to 0.4.4
    * noosphere-car bumped from 0.1.1 to 0.1.2

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.8...noosphere-sphere-v0.6.0) (2023-06-08)


### ⚠ BREAKING CHANGES

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409))
* Migrate blake2b->blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400))

### Features

* Consolidate `NsRecord` implementation in`LinkRecord`. Fixes [#395](https://github.com/subconsciousnetwork/noosphere/issues/395) ([#399](https://github.com/subconsciousnetwork/noosphere/issues/399)) ([9ee4798](https://github.com/subconsciousnetwork/noosphere/commit/9ee47981232fde00b34bb9458c5b0b2799a610ca))
* Migrate blake2b-&gt;blake3 everywhere. ([#400](https://github.com/subconsciousnetwork/noosphere/issues/400)) ([f9e0aec](https://github.com/subconsciousnetwork/noosphere/commit/f9e0aecd76a7253aba13b1881af32a2e543fb6de)), closes [#386](https://github.com/subconsciousnetwork/noosphere/issues/386)


### Bug Fixes

* Enable incremental sphere replication ([#409](https://github.com/subconsciousnetwork/noosphere/issues/409)) ([8812a1e](https://github.com/subconsciousnetwork/noosphere/commit/8812a1e8c9348301b36b77d6c1a2024432806358))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.11.0 to 0.12.0
    * noosphere-storage bumped from 0.6.3 to 0.7.0
    * noosphere-api bumped from 0.7.9 to 0.8.0
    * noosphere-car bumped from 0.1.2 to 0.2.0

## [0.5.8](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.7...noosphere-sphere-v0.5.8) (2023-05-12)


### Features

* Get petnames assigned to a DID for a sphere ([#384](https://github.com/subconsciousnetwork/noosphere/issues/384)) ([aa1cec7](https://github.com/subconsciousnetwork/noosphere/commit/aa1cec7663b41b5bb0f6ffe3066d944b86153b2a))
* Validate petnames and slugs, disallow an empty strings. ([#382](https://github.com/subconsciousnetwork/noosphere/issues/382)) ([fdda233](https://github.com/subconsciousnetwork/noosphere/commit/fdda2330d8545a64054bcda5b97e288c6c7ffdaa))

## [0.5.7](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.6...noosphere-sphere-v0.5.7) (2023-05-11)


### Bug Fixes

* Ensure petname link records are replicated ([#377](https://github.com/subconsciousnetwork/noosphere/issues/377)) ([b5d0204](https://github.com/subconsciousnetwork/noosphere/commit/b5d020423d81ed4e37f33cf6bd0c73b8883b1673))

## [0.5.6](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.5...noosphere-sphere-v0.5.6) (2023-05-09)


### Bug Fixes

* Removed petnames stay removed ([#373](https://github.com/subconsciousnetwork/noosphere/issues/373)) ([76a4ccf](https://github.com/subconsciousnetwork/noosphere/commit/76a4ccfd80f7855933a122a841f0398ab0bcc03c))

## [0.5.4](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.3...noosphere-sphere-v0.5.4) (2023-05-08)


### Features

* Make `anyhow` a workspace dependency in `noosphere-sphere` ([254049b](https://github.com/subconsciousnetwork/noosphere/commit/254049b12a1721a4c024e07dbd46b06737d00ee1))

## [0.5.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.5.2...noosphere-sphere-v0.5.3) (2023-05-08)


### Features

* Enable expired yet valid records in the name system. Update to ucan 0.2.0. ([#360](https://github.com/subconsciousnetwork/noosphere/issues/360)) ([3b0663a](https://github.com/subconsciousnetwork/noosphere/commit/3b0663abc7783a6d33dd47d20caae7597ab93ed0))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.10.2 to 0.11.0
    * noosphere-storage bumped from 0.6.2 to 0.6.3
    * noosphere-api bumped from 0.7.7 to 0.7.8
    * noosphere-ipfs bumped from 0.4.2 to 0.4.3

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.4.1...noosphere-sphere-v0.5.0) (2023-05-02)


### ⚠ BREAKING CHANGES

* Revised tracing configuration (#342)

### Features

* Revised tracing configuration ([#342](https://github.com/subconsciousnetwork/noosphere/issues/342)) ([c4a4084](https://github.com/subconsciousnetwork/noosphere/commit/c4a4084771680c8e49b3db498a5da422db2adda8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.9.3 to 0.10.0
    * noosphere-api bumped from 0.7.4 to 0.7.5
    * noosphere-ipfs bumped from 0.3.4 to 0.4.0

## [0.4.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.4.0...noosphere-sphere-v0.4.1) (2023-04-22)


### Features

* Update IPLD-related dependencies ([#327](https://github.com/subconsciousnetwork/noosphere/issues/327)) ([5fdfadb](https://github.com/subconsciousnetwork/noosphere/commit/5fdfadb1656f9d6eef2dbbb8b00a598106bccf00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.9.2 to 0.9.3
    * noosphere-storage bumped from 0.6.1 to 0.6.2
    * noosphere-api bumped from 0.7.3 to 0.7.4
    * noosphere-ipfs bumped from 0.3.3 to 0.3.4
    * noosphere-car bumped from 0.1.0 to 0.1.1

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.3.2...noosphere-sphere-v0.4.0) (2023-04-19)


### ⚠ BREAKING CHANGES

* Some non-blocking, callback-based C FFI (#322)

### Features

* Some non-blocking, callback-based C FFI ([#322](https://github.com/subconsciousnetwork/noosphere/issues/322)) ([693ce40](https://github.com/subconsciousnetwork/noosphere/commit/693ce40143acf99f758a12df2627e265ef105e03))
* Sphere writes do not block immutable reads ([#321](https://github.com/subconsciousnetwork/noosphere/issues/321)) ([14373c5](https://github.com/subconsciousnetwork/noosphere/commit/14373c5281c091bb41623677571566a2788a7e3f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.9.1 to 0.9.2
    * noosphere-api bumped from 0.7.2 to 0.7.3
    * noosphere-ipfs bumped from 0.3.2 to 0.3.3

## [0.3.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.3.1...noosphere-sphere-v0.3.2) (2023-04-13)


### Bug Fixes

* Unreachable petname sequence is not an error ([#310](https://github.com/subconsciousnetwork/noosphere/issues/310)) ([96f2938](https://github.com/subconsciousnetwork/noosphere/commit/96f2938d76f41fe240466bc7cfe397f886aa7e04))

## [0.3.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.3.0...noosphere-sphere-v0.3.1) (2023-04-10)


### Features

* Dot syntax when traversing by petname ([#306](https://github.com/subconsciousnetwork/noosphere/issues/306)) ([cd87b05](https://github.com/subconsciousnetwork/noosphere/commit/cd87b0533c21bbbd4d82332556e70ecc706a5531))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.9.0 to 0.9.1
    * noosphere-storage bumped from 0.6.0 to 0.6.1
    * noosphere-api bumped from 0.7.1 to 0.7.2
    * noosphere-ipfs bumped from 0.3.1 to 0.3.2

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.2.0...noosphere-sphere-v0.3.0) (2023-04-04)


### ⚠ BREAKING CHANGES

* Apply breaking domain concept in anticipation of beta (#298)

### Miscellaneous Chores

* Apply breaking domain concept in anticipation of beta ([#298](https://github.com/subconsciousnetwork/noosphere/issues/298)) ([bd34ab4](https://github.com/subconsciousnetwork/noosphere/commit/bd34ab49b2d2c65cffe25657cf4d188d5c79d15f))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.8.0 to 0.9.0
    * noosphere-api bumped from 0.7.0 to 0.7.1
    * noosphere-ipfs bumped from 0.3.0 to 0.3.1

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-sphere-v0.1.0...noosphere-sphere-v0.2.0) (2023-03-29)


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
    * noosphere-api bumped from 0.6.0 to 0.7.0
    * noosphere-ipfs bumped from 0.2.0 to 0.3.0

## 0.1.0 (2023-03-14)


### ⚠ BREAKING CHANGES

* Implement C FFI for petname management (#271)
* Petname resolution and synchronization in spheres and gateways (#253)

### Features

* Implement C FFI for petname management ([#271](https://github.com/subconsciousnetwork/noosphere/issues/271)) ([d43c628](https://github.com/subconsciousnetwork/noosphere/commit/d43c6283c6b2374de503d70bd46c8df7d0337c3a))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.6.3 to 0.7.0
    * noosphere-storage bumped from 0.4.2 to 0.5.0
    * noosphere-api bumped from 0.5.6 to 0.6.0
