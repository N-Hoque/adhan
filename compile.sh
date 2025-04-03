#!/bin/bash

set -e

help() {
	echo "This script is used to compile (and optionally deploy) the Adhan player for a specific ARM architecture"
	echo USAGE:
	echo "./compile.sh -t 2 -u test -i 192.168.1.96 -d (Build for Raspberry PI 2 and deploy to the PI at IP 192.168.1.96 for test user)"
	echo "./compile.sh -t 3 -u test -i 192.168.1.96 -d (Same as above but build and deploy for Raspberry PI 3)"
	exit 0
}

while getopts t:u:i:d:h flag; do
	case "${flag}" in
	t) PI_TARGET=${OPTARG} ;;
	u) USER=${OPTARG} ;;
	i) IP=${OPTARG} ;;
	d) DEPLOY="1" ;;
	h) help ;;
	*) ;;
	esac
done

if [[ $PI_TARGET -gt 2 ]]; then
	TARGET_ARCH=aarch64-unknown-linux-gnu
elif [[ $PI_TARGET -eq 2 ]]; then
	TARGET_ARCH=armv7-unknown-linux-gnueabihf
else
	echo "Sorry, this program doesn't currently support the Raspberry PI 0/1"
	exit 2
fi

cross build --profile size --target ${TARGET_ARCH}

if [ -n "$DEPLOY" ]; then
	if [ -z "$USER" ] || [ -z "$IP" ]; then
		echo "Must supply USER and IP to deploy"
		exit 1
	fi
	rsync -vihP target/${TARGET_ARCH}/size/adhan "${USER}@${IP}:adhan_player"
fi

exit 0
