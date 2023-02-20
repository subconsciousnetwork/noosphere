#!/bin/bash

MOUNT_DIR="/home/dhtuser/.noosphere"

KEY=$1
API_PORT=$3
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

if [[ "$API_PORT" ]]; then
	API_ADDRESS="0.0.0.0:${API_PORT}"
	orb-ns run --key ${KEY} --listening-address ${LISTENING_ADDRESS} --api-address ${API_ADDRESS}
else
	orb-ns run --key ${KEY} --listening-address ${LISTENING_ADDRESS}
fi
