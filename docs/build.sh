#!/usr/bin/env bash

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

$SCRIPT_DIR/api/build_rust.sh
$SCRIPT_DIR/api/build_c.sh
