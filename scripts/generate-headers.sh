#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_DIR="$SCRIPT_DIR/../"
cd $PROJECT_DIR

DEFAULT_OUT="$PROJECT_DIR/target/headers"
OUT="$DEFAULT_OUT"

function usage() {
  echo "generate-headers.sh: a utility to generate Noosphere's headers."
  echo "" 
  echo "Usage:" 
  echo "  generate-headers.sh [options]" 
  echo ""
  echo "Options:"
  echo "  -o <file>, --output <file> Output directory."
  echo "                             [default: \$ROOT/target/headers]"
  echo "  -h --help                  Show usage."
  exit 0
}

while [[ $# -gt 0 ]]; do
  case $1 in
    -o|--output)
      OUT="$2"
      shift
      shift
      ;;
    -h|--help)
      usage
      ;;
    -*|--*)
      echo "Unknown option $1"
      exit 1
      ;;
  esac
done

if [[ -f "$OUT" ]]; then
  echo "Output directory must be non-existant or a directory: $OUT"
  exit 1
fi

mkdir -p "$OUT"
cp -r ./rust/noosphere/include "$OUT"

if [[ "$OSTYPE" == "darwin"* ]]; then
  # macos linker fails to generate all exports without this flag
  # https://github.com/subconsciousnetwork/noosphere/issues/473
  CARGO_PROFILE_DEV_CODEGEN_UNITS=1 cargo run --verbose --package noosphere --example generate_header --features headers --locked
else
  cargo run --verbose --package noosphere --example generate_header --features headers --locked
fi

mv ./noosphere.h "$OUT/include/noosphere/noosphere.h"
