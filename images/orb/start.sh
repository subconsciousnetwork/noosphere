#!/bin/bash
set -x

KEY="${1:-${ORB_KEY}}"
COUNTERPART="${2:-${ORB_COUNTERPART}}"
IPFS_API="${3:-${ORB_IPFS_API}}"
NS_API="${4:-${ORB_NS_API}}"

cd /root/sphere

orb key create $KEY

if ! [ -d "./.sphere" ]; then
	orb sphere create --owner-key $KEY
fi

orb config set counterpart $COUNTERPART

ARGS="-i 0.0.0.0"
ARGS="${ARGS} --ipfs-api ${IPFS_API}"

if ! [ -z "$4" ]; then
	ARGS="${ARGS} --name-resolver-api ${NS_API}"
fi

orb serve $ARGS
