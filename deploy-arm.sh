#!/bin/bash

set -e

help() {
	echo "deploy-arm.sh"
	echo "Deploys Adhan binary via SSH to remote ARM system"
	echo ""
	echo "NOTE: This script is based around Raspberry Pi development"
	echo "Target parameter uses Pi version {2, 3} for compilation/deployment"
	echo ""
	echo "USAGE:"
	echo "deploy-arm.sh -t 2 -u test -i 192.168.1.96 (Deploy to IP 192.168.1.96 for test 'user')"
	echo "deploy-arm.sh -t 3 -u test -i 192.168.1.96 (Same as above but build for Raspberry PI 3)"
	exit 0
}

while getopts t:u:i:h flag; do
	case "${flag}" in
	t) PI_TARGET=${OPTARG} ;;
	u) USER=${OPTARG} ;;
	i) IP=${OPTARG} ;;
	h) help ;;
	*) ;;
	esac
done

if [[ "$PI_TARGET" -gt 2 ]]; then
	TARGET_ARCH=aarch64-unknown-linux-gnu
elif [[ "$PI_TARGET" -eq 2 ]]; then
	TARGET_ARCH=armv7-unknown-linux-gnueabihf
else
	echo "Sorry, this program doesn't currently support the Raspberry PI 0/1"
	exit 1
fi

if [ -z "$USER" ] || [ -z "$IP" ]; then
	echo "Must supply USER and IP to deploy"
	exit 2
fi

cross build --profile size --target ${TARGET_ARCH}
rsync -vihP target/${TARGET_ARCH}/size/adhan "${USER}@${IP}:adhan_player"

exit 0
