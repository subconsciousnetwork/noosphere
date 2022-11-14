# Changelog

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0-alpha.1 to 0.1.0

* The following workspace dependencies were updated
  * dependencies
    * noosphere-api bumped from 0.1.0 to 0.2.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.2.1...noosphere-cli-v0.3.0) (2022-11-14)


### ⚠ BREAKING CHANGES

* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/subconsciousnetwork/noosphere/issues/140)) ([af48061](https://github.com/subconsciousnetwork/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.1.0 to 0.2.0
    * noosphere-fs bumped from 0.1.1-alpha.1 to 0.1.0
    * noosphere-api bumped from 0.2.0 to 0.3.0

## [0.2.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.1.1...noosphere-cli-v0.2.0) (2022-11-09)


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

## [0.1.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-cli-v0.1.0-alpha.1...noosphere-cli-v0.1.0) (2022-10-31)


### Features

* **cli:** Find the nearest ancestor sphere ([#119](https://github.com/subconsciousnetwork/noosphere/issues/119)) ([9e33026](https://github.com/subconsciousnetwork/noosphere/commit/9e3302623db3af88df626ccb02ad8fa699e79223))
