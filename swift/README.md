# Noosphere Swift

![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?label=License)
[![Tests](https://img.shields.io/github/workflow/status/subconsciousnetwork/noosphere/Run%20test%20suite/main?label=Tests)](https://github.com/subconsciousnetwork/noosphere/actions/workflows/run_test_suite.yaml?query=branch%3Amain)

The Noosphere Swift workspace contains source code for Swift bindings to Noosphere.

- **[`./swift/Sources/SwiftNoosphere/`](./Sources/SwiftNoosphere)**: Bindings between the C FFI and Swift.
- **[`./swift/Tests/SwiftNoosphereTests/NoosphereTests.swift`](./Tests/SwiftNoosphereTests/NoosphereTests.swift)**: Tests for the Swift module 

:warning: The Noosphere Swift module mostly contains raw bindings to the underlying FFI. Higher-level Swift interfaces will eventually migrate from Subconscious into the module directly over time. :warning:

## Environment Setup

The Noosphere Swift module binds to the Noosphere binary, built from rust. All Noosphere
build dependencies must be installed as a part of the Swift module build process.

Only macOS is supported.

* [Noosphere rust toolchain](../rust/README.md#environment-setup)
* Xcode `xcode-select --install`

## Building

Swift modules linking to binaries must link to an XCFramework, an artifact containing one or several
binaries for multiple platforms.

* Generate headers for the Noosphere library (`noosphere.h`) 
* Generate static libraries for target architectures (e.g. `aarch64-apple-darwin/release/libnoosphere.a`)
* Generate [universal binaries] using [lipo] from the static libraries
* Generate [multiplatform framework bundle] from the universal binaries (`LibNoosphere.xcframework`)

To generate a framework only including `x86_64-apple-darwin`, from the project root, run:

```sh
./scripts/build-framework.sh --lite
```

A framework should be created at `./target/framework/debug/LibNoosphere.xcframework`.


Generating a multiplatform framework (including universal binaries for macOS, iOS simulator, and aarch64-apple-ios) requires building for 5 different platforms so may take some time. For a multiplatform framework, we probably want to use release optimizations, so also set `--release`. From the project root, run:


```sh
./scripts/build-framework.sh --release
```

## Testing

After building the framework, swift tests can be run via:

```sh
./scripts/swift-test.sh
```

If wanting to target the release framework, be sure to use `--release`:

```sh
./scripts/swift-test.sh --release
```

The script temporarily rewrites the `Package.swift` in the project root with the local XCFramework for local development and testing.

[lipo]: https://ss64.com/osx/lipo.html
[universal binaries]: https://en.wikipedia.org/wiki/Fat_binary
[multiplatform framework bundle]: https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle
