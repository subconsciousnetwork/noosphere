# Changelog

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
