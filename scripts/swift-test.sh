#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR="$SCRIPT_DIR/../"
cd $PROJECT_DIR

PLACEHOLDER=./_Package.swift
PROFILE="debug"
SANITIZE=""

function usage() {
  echo "swift-test.sh: a utility to run the Swift Noosphere module tests locally."
  echo "" 
  echo "Usage:" 
  echo "  swift-test.sh [options]" 
  echo ""
  echo "Options:"
  echo "  --release                   Use release build."
  echo "  --sanitize {address,thread} Test with a sanitizer."
  echo "  -h --help                   Show usage."
  exit 0
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --sanitize)
      SANITIZE="$2"
      shift
      shift
      ;;
    --release)
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

trap "mv $PLACEHOLDER Package.swift" EXIT

cp Package.swift $PLACEHOLDER

FRAMEWORK_PATH="./target/framework/$PROFILE/LibNoosphere.xcframework"
sed -i '' -e "s#url: \"[^\"]*\",#path: \"$FRAMEWORK_PATH\"),#" ./Package.swift
sed -i '' -e "s#checksum: \"[^\"]*\"),##" ./Package.swift

# Enable malloc debugging features
# https://developer.apple.com/library/archive/documentation/Performance/Conceptual/ManagingMemory/Articles/MallocDebug.html
if [[ ! -n ${SANITIZE} ]]; then
  swift test -c $PROFILE
else
  MallocPreScribble=1 MallocScribble=1 swift test -c $PROFILE --sanitize=$SANITIZE
fi
exit $?
