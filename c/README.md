# Noosphere C

![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?label=License)
[![Tests](https://img.shields.io/github/workflow/status/subconsciousnetwork/noosphere/Run%20test%20suite/main?label=Tests)](https://github.com/subconsciousnetwork/noosphere/actions/workflows/run_test_suite.yaml?query=branch%3Amain)

An example/test case of binding to the noosphere static lib in C.

## Running

The included Makefile builds the `noosphere.h` header, and a debug build
of `libnoosphere.a` if needed and links to a simple C test case. The test should build and run with either `gcc` or `clang`.

```
cd $NOOSPHERE_REPO/c/example
make clean && make run
```
