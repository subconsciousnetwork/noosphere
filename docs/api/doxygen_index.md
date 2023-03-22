# Noosphere

Documentation for [Noosphere]'s C API and FFI layer.

See an [example case](https://github.com/subconsciousnetwork/noosphere/blob/main/c/example/main.c) in the repository.

## Overview

All functions in noosphere.h return owned data, unless otherwise specified.
That means you _must_ manually release all data returned by these functions,
using the corresponding "free" function, e.g. `ns_string_free()` to release
a `char *` returned from a Noosphere function.

[Noosphere]: https://github.com/subconsciousnetwork/noosphere

