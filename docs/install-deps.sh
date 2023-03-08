#!/usr/bin/env bash

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

YUM_CMD=$(which yum)
APT_GET_CMD=$(which apt-get)
BREW_CMD=$(which brew)

if [[ ! -z $APT_GET_CMD ]]; then
	sudo apt install doxygen
elif [[ ! -z $YUM_CMD ]]; then
	yum install doxygen
elif [[ ! -z $BREW_CMD ]]; then
	brew install doxygen
else
    echo -n "Could not find suitable package manager. You must manually install dependencies."
    exit 1
fi
