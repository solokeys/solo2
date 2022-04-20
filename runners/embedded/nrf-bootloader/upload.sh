#!/usr/bin/env bash

set -euxo pipefail

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 update_filename serial_port"
    exit 0
fi

UPDATE_FILENAME=$1
SERIAL_PORT=$2

nrfutil dfu usb-serial -pkg $UPDATE_FILENAME -p $SERIAL_PORT
