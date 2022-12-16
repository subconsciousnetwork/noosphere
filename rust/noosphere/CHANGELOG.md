# Changelog

## [0.15.0](https://github.com/cdata/noosphere/compare/noosphere-v0.14.0...noosphere-v0.15.0) (2022-12-16)


### ⚠ BREAKING CHANGES

* Hoot!
* Such comment
* Blah blah
* WAT
* This is another comment
* Commentz
* commmmment
* So many comments
* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Blah blah ([e0b13c2](https://github.com/cdata/noosphere/commit/e0b13c2f9148a3399057a5e7f99136df1ec293c9))
* Commentz ([ebdac27](https://github.com/cdata/noosphere/commit/ebdac27ae45a9dd5b147285fb888faf094f44555))
* commmmment ([4f4416d](https://github.com/cdata/noosphere/commit/4f4416d3c54c783aa6b63503e4e18799faa6e696))
* Hoot! ([5a2adb9](https://github.com/cdata/noosphere/commit/5a2adb993cba52ed61bb55cc2d08acf6fb4feae2))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* So many comments ([1ea6e04](https://github.com/cdata/noosphere/commit/1ea6e047ba3504e10c21c52b82928ba488815bb2))
* Such comment ([9d2731b](https://github.com/cdata/noosphere/commit/9d2731b6c6f7df015fde82bd17a8b99cd2bd8ae8))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))
* This is another comment ([e320c2c](https://github.com/cdata/noosphere/commit/e320c2c4529cd23992bad0fdb670dd781f4abdd7))
* WAT ([1cbf3a0](https://github.com/cdata/noosphere/commit/1cbf3a0e4b1dc95d2b252e2806cb1f4589026640))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.5.0 to 0.5.1

## [0.14.0](https://github.com/cdata/noosphere/compare/noosphere-v0.13.0...noosphere-v0.14.0) (2022-12-16)


### ⚠ BREAKING CHANGES

* Such comment
* Blah blah
* WAT
* This is another comment
* Commentz
* commmmment
* So many comments
* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Blah blah ([e0b13c2](https://github.com/cdata/noosphere/commit/e0b13c2f9148a3399057a5e7f99136df1ec293c9))
* Commentz ([ebdac27](https://github.com/cdata/noosphere/commit/ebdac27ae45a9dd5b147285fb888faf094f44555))
* commmmment ([4f4416d](https://github.com/cdata/noosphere/commit/4f4416d3c54c783aa6b63503e4e18799faa6e696))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* So many comments ([1ea6e04](https://github.com/cdata/noosphere/commit/1ea6e047ba3504e10c21c52b82928ba488815bb2))
* Such comment ([9d2731b](https://github.com/cdata/noosphere/commit/9d2731b6c6f7df015fde82bd17a8b99cd2bd8ae8))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))
* This is another comment ([e320c2c](https://github.com/cdata/noosphere/commit/e320c2c4529cd23992bad0fdb670dd781f4abdd7))
* WAT ([1cbf3a0](https://github.com/cdata/noosphere/commit/1cbf3a0e4b1dc95d2b252e2806cb1f4589026640))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.13.0](https://github.com/cdata/noosphere/compare/noosphere-v0.12.0...noosphere-v0.13.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* WAT

### Features

* WAT ([1cbf3a0](https://github.com/cdata/noosphere/commit/1cbf3a0e4b1dc95d2b252e2806cb1f4589026640))

## [0.12.0](https://github.com/cdata/noosphere/compare/noosphere-v0.11.0...noosphere-v0.12.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* This is another comment
* Commentz
* commmmment
* So many comments
* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Commentz ([ebdac27](https://github.com/cdata/noosphere/commit/ebdac27ae45a9dd5b147285fb888faf094f44555))
* commmmment ([4f4416d](https://github.com/cdata/noosphere/commit/4f4416d3c54c783aa6b63503e4e18799faa6e696))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* So many comments ([1ea6e04](https://github.com/cdata/noosphere/commit/1ea6e047ba3504e10c21c52b82928ba488815bb2))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))
* This is another comment ([e320c2c](https://github.com/cdata/noosphere/commit/e320c2c4529cd23992bad0fdb670dd781f4abdd7))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.11.0](https://github.com/cdata/noosphere/compare/noosphere-v0.10.0...noosphere-v0.11.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* Commentz

### Features

* Commentz ([ebdac27](https://github.com/cdata/noosphere/commit/ebdac27ae45a9dd5b147285fb888faf094f44555))

## [0.10.0](https://github.com/cdata/noosphere/compare/noosphere-v0.9.0...noosphere-v0.10.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* commmmment
* So many comments
* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* commmmment ([4f4416d](https://github.com/cdata/noosphere/commit/4f4416d3c54c783aa6b63503e4e18799faa6e696))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* So many comments ([1ea6e04](https://github.com/cdata/noosphere/commit/1ea6e047ba3504e10c21c52b82928ba488815bb2))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.9.0](https://github.com/cdata/noosphere/compare/noosphere-v0.8.0...noosphere-v0.9.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* So many comments
* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* So many comments ([1ea6e04](https://github.com/cdata/noosphere/commit/1ea6e047ba3504e10c21c52b82928ba488815bb2))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.8.0](https://github.com/cdata/noosphere/compare/noosphere-v0.7.0...noosphere-v0.8.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* More comments in file
* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* More comments in file ([5607ee9](https://github.com/cdata/noosphere/commit/5607ee904431a0f8306977b79800ce534d6f3082))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.7.0](https://github.com/cdata/noosphere/compare/noosphere-v0.6.0...noosphere-v0.7.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* New comment in file
* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* New comment in file ([4071de4](https://github.com/cdata/noosphere/commit/4071de4eaac23db901de93a7294833ba08ef9ea6))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))

## [0.6.0](https://github.com/cdata/noosphere/compare/noosphere-v0.5.0...noosphere-v0.6.0) (2022-12-15)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.
* Several critical dependencies of this library were updated to new versions that contain breaking changes.
* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`
* Some FFI interfaces now have simplified interfaces.
* Many APIs that previously asked for bare strings when a DID string was expected now expect a newtype called `Did` that wraps a string.
* The `noosphere-api` Client now holds an owned key instead of a reference.

### Features

* `SphereFs` is initialized with key material ([#140](https://github.com/cdata/noosphere/issues/140)) ([af48061](https://github.com/cdata/noosphere/commit/af4806114ca8f7703e0a888c7f369a4a4ed69c00))
* Add `noosphere` crate-based Swift package ([#131](https://github.com/cdata/noosphere/issues/131)) ([e1204c2](https://github.com/cdata/noosphere/commit/e1204c2a5822c3c0dbb7e61bbacffb2c1f49d8d8))
* Add `SphereFS` read/write to FFI ([#141](https://github.com/cdata/noosphere/issues/141)) ([26e34ac](https://github.com/cdata/noosphere/commit/26e34acfe70cac099acfa6dc8c2cf156c46fdae0))
* Beautify the Sphere Viewer demo app ([#186](https://github.com/cdata/noosphere/issues/186)) ([3e30fdb](https://github.com/cdata/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce pet names to spheres ([#154](https://github.com/cdata/noosphere/issues/154)) ([7495796](https://github.com/cdata/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/cdata/noosphere/issues/182)) ([44170a2](https://github.com/cdata/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))
* **noosphere:** Introduce `noosphere` crate ([#123](https://github.com/cdata/noosphere/issues/123)) ([ad9daa6](https://github.com/cdata/noosphere/commit/ad9daa697067069197d12ee8e7f11bdbedc3662d))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/cdata/noosphere/issues/162)) ([1e83bbb](https://github.com/cdata/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/cdata/noosphere/issues/178)) ([4db55c4](https://github.com/cdata/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/cdata/noosphere/issues/177)) ([e269e04](https://github.com/cdata/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/cdata/noosphere/issues/180)) ([1a1114b](https://github.com/cdata/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.4.0 to 0.5.0

## [0.5.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.4.0...noosphere-v0.5.0) (2022-12-14)


### ⚠ BREAKING CHANGES

* `SphereFile` fields referring to a `revision` now refer to a `version` instead.

### Features

* Beautify the Sphere Viewer demo app ([#186](https://github.com/subconsciousnetwork/noosphere/issues/186)) ([3e30fdb](https://github.com/subconsciousnetwork/noosphere/commit/3e30fdb5e2b6758397f05343491a36512a4f4a0c))
* Introduce web bindings and `orb` NPM package ([#182](https://github.com/subconsciousnetwork/noosphere/issues/182)) ([44170a2](https://github.com/subconsciousnetwork/noosphere/commit/44170a27be2e1d180b1cee153937ab2cef16a591))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-into bumped from 0.3.0 to 0.4.0

## [0.4.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.3.0...noosphere-v0.4.0) (2022-11-30)


### ⚠ BREAKING CHANGES

* Several critical dependencies of this library were updated to new versions that contain breaking changes.

### Miscellaneous Chores

* Update IPLD-adjacent dependencies ([#180](https://github.com/subconsciousnetwork/noosphere/issues/180)) ([1a1114b](https://github.com/subconsciousnetwork/noosphere/commit/1a1114b0c6277ea2c0d879e43191e962eb2e462b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.3.0 to 0.4.0
    * noosphere-fs bumped from 0.2.0 to 0.3.0
    * noosphere-storage bumped from 0.2.0 to 0.3.0
    * noosphere-api bumped from 0.4.0 to 0.5.0

## [0.3.0](https://github.com/subconsciousnetwork/noosphere/compare/noosphere-v0.2.0...noosphere-v0.3.0) (2022-11-29)


### ⚠ BREAKING CHANGES

* The `StorageProvider` trait has been replaced by the `Storage` trait. This new trait allows for distinct backing implementations of `BlockStore` and `KeyValueStore`.
* The `.sphere` directory has a new layout; the files previously used to store metadata have been replaced with database metadata; the `blocks` directory is now called `storage`. At this time the easiest migration path is to initialize a new sphere and copy your existing files into it.
* `SphereIpld` identity is now a `Did`

### Features

* Introduce pet names to spheres ([#154](https://github.com/subconsciousnetwork/noosphere/issues/154)) ([7495796](https://github.com/subconsciousnetwork/noosphere/commit/74957968af7f7e51a6aa731192431fbf5e01215e))
* Re-implement `noosphere-cli` in terms of `noosphere` ([#162](https://github.com/subconsciousnetwork/noosphere/issues/162)) ([1e83bbb](https://github.com/subconsciousnetwork/noosphere/commit/1e83bbb689642b878f4f6909d7dd4a6df56b29f9))
* Refactor storage interfaces ([#178](https://github.com/subconsciousnetwork/noosphere/issues/178)) ([4db55c4](https://github.com/subconsciousnetwork/noosphere/commit/4db55c4cba56b329a638a4227e7f3247ad8d319c))
* Syndicate sphere revisions to IPFS Kubo ([#177](https://github.com/subconsciousnetwork/noosphere/issues/177)) ([e269e04](https://github.com/subconsciousnetwork/noosphere/commit/e269e0484b73e0f5507406d57a2c06cf849bee3d))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * noosphere-core bumped from 0.2.0 to 0.3.0
    * noosphere-fs bumped from 0.1.0 to 0.2.0
    * noosphere-storage bumped from 0.1.0 to 0.2.0
    * noosphere-api bumped from 0.3.0 to 0.4.0

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
