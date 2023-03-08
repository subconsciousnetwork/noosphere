# Noosphere Docs

This directory contains generators for each supported language's Noosphere binding. Each language attempts to use idiomatic documentation and tooling for its environment.

* Rust ([RustDoc]) (use [docs.rs](https://docs.rs))
* C ([Doxygen])
* Swift TBD ([Jazzy])
* JavaScript TBD ([JsDoc])

## Dependencies

In addition to the Rust tools for building Noosphere, additional dependencies are needed
to build docs. Run `./install-deps.sh`, or manually install the dependencies:

* [Doxygen]

## Building

Run `./build.sh` to build docs for all supported languages. Static HTML is emitted in `./docs/out`.

[RustDoc]: https://doc.rust-lang.org/rustdoc/index.html 
[Doxygen]: https://www.doxygen.nl/
[Jazzy]: https://github.com/realm/jazzy 
[JsDoc]: https://jsdoc.app/
