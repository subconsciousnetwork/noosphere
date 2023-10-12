#!/bin/bash
set -ex

KEY="${1:-${ORB_KEY}}"
COUNTERPART="${2:-${ORB_COUNTERPART}}"
IPFS_API="${3:-${ORB_IPFS_API}}"
NS_API="${4:-${ORB_NS_API}}"

cd /root/sphere

orb key create $KEY

if ! [ -d "./.sphere" ]; then
	orb sphere create --owner-key $KEY
fi

orb sphere config set counterpart $COUNTERPART

ARGS="-i 0.0.0.0"
ARGS="${ARGS} --ipfs-api ${IPFS_API}"
ARGS="${ARGS} --storage-memory-cache-limit 50000000" # ~50MB storage memory cache limit

if ! [ -z "$NS_API" ]; then
	ARGS="${ARGS} --name-resolver-api ${NS_API}"
fi

echo "RUST_LOG=${RUST_LOG}"
echo "NOOSPHERE_LOG=${NOOSPHERE_LOG}"

orb serve $ARGS
