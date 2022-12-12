#!/bin/bash

set -e

# See: https://stackoverflow.com/a/246128
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PACKAGE_JSON_FILE="$SCRIPT_DIR/../package.json"
INDEX_HTML_FILE="$SCRIPT_DIR/../dist/index.html"
SPHERE_VIEWER_VERSION=`cat $PACKAGE_JSON_FILE | jq .version | xargs echo`
SPHERE_VIEWER_SHA=`git rev-parse HEAD`

sed -i -e "s#SPHERE_VIEWER_VERSION = \"0.0.0\"#SPHERE_VIEWER_VERSION = \"$SPHERE_VIEWER_VERSION\"#" $INDEX_HTML_FILE
sed -i -e "s#SPHERE_VIEWER_SHA = \"abcdef\"#SPHERE_VIEWER_SHA = \"${SPHERE_VIEWER_SHA:0:6}\"#" $INDEX_HTML_FILE

set +e