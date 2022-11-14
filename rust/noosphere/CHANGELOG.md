# Changelog

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0...noosphere-v0.2.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/subconsciousnetwork/noosphere/issues/141)) ([26e34ac](https://github.com/subconsciousnetwork/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0
    * noosphere-fs bumped from 0.1.1-alpha.1 to 0.1.0
    * noosphere-api bumped from 0.2.0 to 0.3.0

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0-alpha.1...noosphere-v0.1.0) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0 to 0.2.0

## [0.1.0-alpha.1](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.1.0-alpha.1...noosphere-v0.1.0-alpha.1) (2022-11-09)


### ⚠ BREAKING CHANGES

* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* Add `noosphere` crate-based Swift package ([#131](https://github.com/subconsciousnetwork/noosphere/issues/131)) ([e1204c2](https://github.com/subconsciousnetwork/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0-alpha.1 to 0.1.0
    * noosphere-fs bumped from 0.1.0-alpha.1 to 0.1.1-alpha.1
    * noosphere-storage bumped from 0.1.0-alpha.1 to 0.1.0

## 0.1.0-alpha.1 (2022-11-03)


### Features

* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/subconsciousnetwork/noosphere/issues/123)) ([ad9daa6](https://github.com/subconsciousnetwork/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0-alpha.1 to 0.1.0
