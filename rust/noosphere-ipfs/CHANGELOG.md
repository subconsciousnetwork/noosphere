# Changelog

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-storage bumped from 0.4.0 to 0.4.2

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.8.0 to 0.9.0

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.9.1 to 0.9.2

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.10.0 to 0.10.1

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.10.1 to 0.10.2

* The following workspace dependencies were updated
  * dependencies
    * noosphere-car bumped from 0.1.1 to 0.1.2

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.12.0 to 0.12.1

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.12.1 to 0.12.2

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.12.2 to 0.12.3

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.0 to 0.7.1
  * dev-dependencies
    * noosphere-storage bumped from 0.7.0 to 0.7.1
    * noosphere-core bumped from 0.13.0 to 0.13.1

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.13.2 to 0.14.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.7.1 to 0.8.0
  * dev-dependencies
    * noosphere-storage bumped from 0.7.1 to 0.8.0
    * noosphere-core bumped from 0.14.0 to 0.15.0

## [0.7.4](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.7.3...noosphere-ipfs-v0.7.4) (2023-08-29)


### Bug Fixes

* Better handling of removed content in `orb` ([#588](https://github.com/subconsciousnetwork/noosphere/issues/588)) ([b811e68](https://github.com/subconsciousnetwork/noosphere/commit/b811e6891aec648d9a856adaeda86335ae94cacb))
* Increase allowed request body payload size ([#608](https://github.com/subconsciousnetwork/noosphere/issues/608)) ([da83f38](https://github.com/subconsciousnetwork/noosphere/commit/da83f3894d47d606bd148b72db83414a92688cf4))


### Dependencies

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.15.1 to 0.15.2

## [0.7.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.7.2...noosphere-ipfs-v0.7.3) (2023-08-10)


### Features

* `orb sphere history` and `orb sphere render` ([#576](https://github.com/subconsciousnetwork/noosphere/issues/576)) ([a6f0a74](https://github.com/subconsciousnetwork/noosphere/commit/a6f0a74cde2fc001bfff5c1bed0844ac19fc8258))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.8.0 to 0.8.1
  * dev-dependencies
    * noosphere-storage bumped from 0.8.0 to 0.8.1
    * noosphere-core bumped from 0.15.0 to 0.15.1

## [0.7.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.6.1...noosphere-ipfs-v0.7.0) (2023-07-19)


### ⚠ BREAKING CHANGES

* Replace `noosphere-car` with `iroh-car` throughout the Noosphere crates. ([#492](https://github.com/subconsciousnetwork/noosphere/issues/492))

### Features

* Replace `noosphere-car` with `iroh-car` throughout the Noosphere crates. ([#492](https://github.com/subconsciousnetwork/noosphere/issues/492)) ([e89d498](https://github.com/subconsciousnetwork/noosphere/commit/e89d49879b3a1d2ce8529e438df7995ae8b4e44f))


### Dependencies

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.13.1 to 0.13.2

## [0.6.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.5.3...noosphere-ipfs-v0.6.0) (2023-07-01)


### ⚠ BREAKING CHANGES

* Update to `rs-ucan` 0.4.0, implementing UCAN 0.10ish. ([#449](https://github.com/subconsciousnetwork/noosphere/issues/449))

### Features

* Update to `rs-ucan` 0.4.0, implementing UCAN 0.10ish. ([#449](https://github.com/subconsciousnetwork/noosphere/issues/449)) ([8b806c5](https://github.com/subconsciousnetwork/noosphere/commit/8b806c5462b5601a5f8417a6a20769b76b57ee6a))


### Dependencies

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.12.3 to 0.13.0

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.4.4...noosphere-ipfs-v0.5.0) (2023-06-08)


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
    * noosphere-storage bumped from 0.6.3 to 0.7.0
  * dev-dependencies
    * noosphere-storage bumped from 0.6.3 to 0.7.0
    * noosphere-car bumped from 0.1.2 to 0.2.0
    * noosphere-core bumped from 0.11.0 to 0.12.0

## [0.4.3](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.4.2...noosphere-ipfs-v0.4.3) (2023-05-08)


### Features

* Enable expired yet valid records in the name system. Update to ucan 0.2.0. ([#360](https://github.com/subconsciousnetwork/noosphere/issues/360)) ([3b0663a](https://github.com/subconsciousnetwork/noosphere/commit/3b0663abc7783a6d33dd47d20caae7597ab93ed0))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.2 to 0.6.3
  * dev-dependencies
    * noosphere-storage bumped from 0.6.2 to 0.6.3
    * noosphere-core bumped from 0.10.2 to 0.11.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.3.4...noosphere-ipfs-v0.4.0) (2023-05-02)


### ⚠ BREAKING CHANGES

* Revised tracing configuration (#342)

### Features

* Revised tracing configuration ([#342](https://github.com/subconsciousnetwork/noosphere/issues/342)) ([c4a4084](https://github.com/subconsciousnetwork/noosphere/commit/c4a4084771680c8e49b3db498a5da422db2adda8))


### Dependencies

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.9.3 to 0.10.0

## [0.3.4](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.3.3...noosphere-ipfs-v0.3.4) (2023-04-22)


### Features

* Update IPLD-related dependencies ([#327](https://github.com/subconsciousnetwork/noosphere/issues/327)) ([5fdfadb](https://github.com/subconsciousnetwork/noosphere/commit/5fdfadb1656f9d6eef2dbbb8b00a598106bccf00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-car bumped from 0.1.0 to 0.1.1
  * dev-dependencies
    * noosphere-storage bumped from 0.6.1 to 0.6.2
    * noosphere-core bumped from 0.9.2 to 0.9.3

## [0.3.2](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.3.1...noosphere-ipfs-v0.3.2) (2023-04-10)


### Features

* Add instrumentation to `noosphere-ns` and `noosphere-ipfs`. ([#304](https://github.com/subconsciousnetwork/noosphere/issues/304)) ([3d6062d](https://github.com/subconsciousnetwork/noosphere/commit/3d6062d501e21393532b2db6f9ac740a041d91ba))
* Dot syntax when traversing by petname ([#306](https://github.com/subconsciousnetwork/noosphere/issues/306)) ([cd87b05](https://github.com/subconsciousnetwork/noosphere/commit/cd87b0533c21bbbd4d82332556e70ecc706a5531))


### Bug Fixes

* Introduce `TryOrReset` to help worker threads ([#300](https://github.com/subconsciousnetwork/noosphere/issues/300)) ([5ea4b2c](https://github.com/subconsciousnetwork/noosphere/commit/5ea4b2c91d0b829e22f0c0b3cd22fe837eddf905))
* Several fixes for noosphere-ipfs as it gets further integrated ([#302](https://github.com/subconsciousnetwork/noosphere/issues/302)) ([9da4dd0](https://github.com/subconsciousnetwork/noosphere/commit/9da4dd063edf5bbf1a86556db64428d2ecb43f79))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.6.0 to 0.6.1
  * dev-dependencies
    * noosphere-storage bumped from 0.6.0 to 0.6.1
    * noosphere-core bumped from 0.9.0 to 0.9.1

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.2.0...noosphere-ipfs-v0.3.0) (2023-03-29)


### ⚠ BREAKING CHANGES

* Traverse the Noosphere vast (#284)
* Fork `iroh-car` as `noosphere-car` (#283)

### Features

* Fork `iroh-car` as `noosphere-car` ([#283](https://github.com/subconsciousnetwork/noosphere/issues/283)) ([b0b7c38](https://github.com/subconsciousnetwork/noosphere/commit/b0b7c3835ff1ef271bbe0f833f6f7856fcc30de1))
* Traverse the Noosphere vast ([#284](https://github.com/subconsciousnetwork/noosphere/issues/284)) ([43bceaf](https://github.com/subconsciousnetwork/noosphere/commit/43bceafcc838c5b06565780f372bf7b401de288e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.5.0 to 0.6.0
  * dev-dependencies
    * noosphere-storage bumped from 0.5.0 to 0.6.0
    * noosphere-core bumped from 0.7.0 to 0.8.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.1.2...noosphere-ipfs-v0.2.0) (2023-03-14)


### ⚠ BREAKING CHANGES

* Petname resolution and synchronization in spheres and gateways (#253)
* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. (#254)
* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. (#252)

### Features

* Implement `IpfsClient::get_block` for Kubo for orb/orb-ns integration with IPFS. ([#251](https://github.com/subconsciousnetwork/noosphere/issues/251)) ([f18db24](https://github.com/subconsciousnetwork/noosphere/commit/f18db2425d620165090afee9418d5f743a0cf579))
* Introduce `noosphere-gateway` crate ([#238](https://github.com/subconsciousnetwork/noosphere/issues/238)) ([791bc39](https://github.com/subconsciousnetwork/noosphere/commit/791bc3996cfac12cb077c3721f22d080a71d33ba))
* Petname resolution and synchronization in spheres and gateways ([#253](https://github.com/subconsciousnetwork/noosphere/issues/253)) ([f7ddfa7](https://github.com/subconsciousnetwork/noosphere/commit/f7ddfa7b65129efe795c6e3fca58cdc22799127a))


### Bug Fixes

* Reconfigure module dependencies so that noosphere-ipfs depends on noosphere-storage, and not the other way around creating a cycle. ([#254](https://github.com/subconsciousnetwork/noosphere/issues/254)) ([b79872a](https://github.com/subconsciousnetwork/noosphere/commit/b79872afd54c7b69d447dfe99e750bb6a813645c))


### Miscellaneous Chores

* Templatize the two IPFS HTTP APIs as noosphere_ipfs::IpfsClient, and reconfigure KuboStorage as IpfsStorage, operating on IpfsClient rather than a URL. ([#252](https://github.com/subconsciousnetwork/noosphere/issues/252)) ([518beae](https://github.com/subconsciousnetwork/noosphere/commit/518beae563bd04c921ee3c6641a7249f14c611e4))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-storage bumped from 0.4.2 to 0.5.0
  * dev-dependencies
    * noosphere-storage bumped from 0.4.2 to 0.5.0
    * noosphere-core bumped from 0.6.3 to 0.7.0

## [0.1.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-ipfs-v0.1.0...noosphere-ipfs-v0.1.1) (2023-01-31)


### Bug Fixes

* Enable `noosphere-ipfs` to compile on its own ([764eeb7](https://github.com/subconsciousnetwork/noosphere/commit/764eeb7d24df2773afd5bce934f2de6fc2de2640))

## 0.1.0 (2023-01-31)


### Features

* Introduce `noosphere-ipfs` crate ([#203](https://github.com/subconsciousnetwork/noosphere/issues/203)) ([ad1945b](https://github.com/subconsciousnetwork/noosphere/commit/ad1945bb7d64f169b6dac96807bf8d8e0c3ab482))
