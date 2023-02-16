#!/bin/bash

KEY=$1
COUNTERPART=$2

cd /root/sphere

orb key create $KEY

if ! [ -d "./.sphere" ]; then
  orb sphere create --owner-key $KEY
fi

orb config set counterpart $COUNTERPART
orb serve -i 0.0.0.0