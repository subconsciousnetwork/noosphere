#!/bin/bash

MOUNT_DIR="/home/dhtuser/.noosphere"
if [[ ! -d "$MOUNT_DIR" ]]; then
	echo "Missing mount on $MOUNT_DIR."
	exit 1
fi
if [[ -z "$1" ]]; then
	echo "ARGS: KEY [SWARM_PORT] [API_PORT]"	
	exit 1
fi

KEY=$1
API_PORT=$3
SWARM_PORT=6666
if [[ "$2" ]]; then
	SWARM_PORT="$2"
fi
LISTENING_ADDRESS="0.0.0.0:${SWARM_PORT}"

cd /home/dhtuser

if [[ "$API_PORT" ]]; then
	orb-ns run --key ${KEY} --listening-address ${LISTENING_ADDRESS} --api-address ${API_PORT}
else
	orb-ns run --key ${KEY} --listening-address ${LISTENING_ADDRESS}
fi
