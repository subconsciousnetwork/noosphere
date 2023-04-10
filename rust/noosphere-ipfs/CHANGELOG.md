# Changelog

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-storage bumped from 0.4.0 to 0.4.2

* The following workspace dependencies were updated
  * dev-dependencies
    * noosphere-core bumped from 0.8.0 to 0.9.0

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
