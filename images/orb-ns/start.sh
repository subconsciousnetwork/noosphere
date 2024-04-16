#!/bin/bash
set -x

MOUNT_DIR="/home/dhtuser/.noosphere"
ORB_NS="/usr/bin/orb-ns"

cd /home/dhtuser

if [[ "$KEY_NAME" = "ephemeral" ]]; then
	# This has a side effect of ensuring a ~/.noosphere/keys directory
	# exists to store the key, circumventing the need for mounting 
	$ORB_NS key-gen --key ephemeral
fi
if [[ ! -d "$MOUNT_DIR" ]]; then
	echo "Missing mount on $MOUNT_DIR."
	exit 1
fi

echo "RUST_LOG=${RUST_LOG}"
echo "NOOSPHERE_LOG=${NOOSPHERE_LOG}"
echo "KEY_NAME=${KEY_NAME}"
echo "P2P_ADDRESS=${P2P_ADDRESS}"
echo "API_ADDRESS=${API_ADDRESS}"
echo "IPFS_URL=${IPFS_URL}"

$ORB_NS run --key "${KEY_NAME}" \
	--listening-address "${P2P_ADDRESS}" \
	--api-address "${API_ADDRESS}" \
	--ipfs-api-url "${IPFS_URL}"
