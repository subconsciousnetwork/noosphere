#!/bin/bash
set -x

MOUNT_DIR="/home/dhtuser/.noosphere"

KEY=$1
CONFIG_FILE=$2

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
	echo "ARGS: KEY CONFIG_FILE"	
	exit 1
fi
if [[ -z "$2" ]]; then
	echo "Missing config file path."
	exit 1
fi

ls -al /home/dhtuser
cat /home/dhtuser/orb-ns.config.toml

orb-ns run --config $CONFIG_FILE
