#!/bin/bash

MOUNT_DIR="/home/dhtuser/.noosphere"

KEY=$1
IPFS_API_URL=$3
API_PORT=$4
SWARM_PORT=6666
if [[ "$2" ]]; then
	SWARM_PORT="$2"
fi

cd /home/dhtuser

if [[ "$KEY" = "ephemeral" ]]; then
	# This has a side effect of ensuring a ~/.noosphere/keys directory
	# exists to store the key, circumventing the need for mounting 
	orb-ns key-gen --key ephemeral
fi
if [[ ! -d "$MOUNT_DIR" ]]; then
	echo "Missing mount on $MOUNT_DIR."
	exit 1
fi
if [[ -z "$1" ]]; then
	echo "ARGS: KEY [SWARM_PORT] [API_PORT]"	
	exit 1
fi

LISTENING_ADDRESS="0.0.0.0:${SWARM_PORT}"

ARGS=""
ARGS="${ARGS} --key ${KEY}"
ARGS="${ARGS} --listening-address ${LISTENING_ADDRESS}"
if [[ "$API_PORT" ]]; then
	ARGS="${ARGS} --api-address 0.0.0.0:${API_PORT}"
fi
if [[ "$IPFS_API_URL" ]]; then
	ARGS="${ARGS} --ipfs-api-url ${IPFS_API_URL}"
fi

orb-ns run ${ARGS}
