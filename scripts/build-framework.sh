#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR="$SCRIPT_DIR/../"
cd $PROJECT_DIR

OUT=""
LITE=0
RELEASE_FLAG=""
PROFILE="debug"

function usage() {
  echo "build-framework.sh: a utility to build an Apple XCFramework for Noosphere."
  echo "" 
  echo "Usage:" 
  echo "  build-framework.sh [options]" 
  echo ""
  echo "Options:"
  echo "  -l --lite                  Only include x86_64-apple-darwin in framework."
  echo "  -o <file>, --output <file> Output file."
  echo "                             [default: \$ROOT/target/framework/{release,debug}/LibNoosphere.xcframework]"
  echo "  --release                  Build artifacts in release-mode."
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
    -l|--lite)
      LITE=1
      shift
      ;;
    --release)
      RELEASE_FLAG="--release"
      PROFILE="release"
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

UNAME=$(uname -m)
if [[ $UNAME == "aarch64" ]]; then
  HOST_PLATFORM="aarch64-apple-darwin"
  OTHER_PLATFORM="x86_64-apple-darwin"
elif [[ $UNAME == "x86_64" ]]; then
  HOST_PLATFORM="x86_64-apple-darwin"
  OTHER_PLATFORM="aarch64-apple-darwin"
else
  echo "Unknown host platform: $UNAME"
  exit 1
fi

if [[ ! -n ${OUT} ]]; then
  OUT="$PROJECT_DIR/target/framework/$PROFILE/LibNoosphere.xcframework"
fi

rm -rf "$OUT"

$SCRIPT_DIR/generate-headers.sh

TARGETS=($HOST_PLATFORM)
if [[ $LITE -eq 0 ]]; then
  TARGETS+=($OTHER_PLATFORM)
  TARGETS+=("aarch64-apple-ios")
  TARGETS+=("x86_64-apple-ios")
  TARGETS+=("aarch64-apple-ios-sim")
fi

for TARGET in "${TARGETS[@]}"; do
  rustup target install $TARGET
  cargo build --package noosphere $RELEASE_FLAG --target $TARGET --locked
done

if [[ $LITE -eq 0 ]]; then
  mkdir -p ./target/{macos-universal,simulator-universal}/$PROFILE

  lipo -create \
    "./target/x86_64-apple-darwin/$PROFILE/libnoosphere.a" \
    "./target/aarch64-apple-darwin/$PROFILE/libnoosphere.a" \
    -output "./target/macos-universal/$PROFILE/libnoosphere.a"

  lipo -create \
    "./target/x86_64-apple-ios/$PROFILE/libnoosphere.a" \
    "./target/aarch64-apple-ios-sim/$PROFILE/libnoosphere.a" \
    -output "./target/simulator-universal/$PROFILE/libnoosphere.a"

  xcodebuild -create-xcframework \
    -library "./target/macos-universal/$PROFILE/libnoosphere.a" \
    -headers ./target/headers/include/ \
    -library "./target/simulator-universal/$PROFILE/libnoosphere.a" \
    -headers ./target/headers/include/ \
    -library "./target/aarch64-apple-ios/$PROFILE/libnoosphere.a" \
    -headers ./target/headers/include/ \
    -output "$OUT"
else
  xcodebuild -create-xcframework \
    -library "./target/$HOST_PLATFORM/$PROFILE/libnoosphere.a" \
    -headers ./target/headers/include/ \
    -output "$OUT"
fi
