#!/usr/bin/env bash

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
RUST_SRC="$SCRIPT_DIR/../../rust/noosphere"
cd $RUST_SRC
cargo run --features=headers --example generate_header
cp noosphere.h "$SCRIPT_DIR/noosphere.h"
cd "$SCRIPT_DIR"

doxygen noosphere.doxygen
rm "$SCRIPT_DIR/noosphere.h"
