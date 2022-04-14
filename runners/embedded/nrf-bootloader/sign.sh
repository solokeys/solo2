#!/usr/bin/env bash

set -e

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 app_version output_filename input_hex_app"
    exit 0
fi

# The update package (a zip archive)
OUTPUT_FILE=$2

# The application version is used to prevent downgrading
APP_VERSION=$1

APP_HEX_FILE=$3

if [[ "$APP_HEX_FILE" == *".ihex"* ]]; then
	echo "Error: input hex-file with suffix .ihex not allowed!"
	echo "Error: please use a .hex suffix"
	exit 1
fi


# The hardware version is used to prevent accidently flashing a signed update package for another product
HW_VERSION=52

# The softdevice version is used to make sure the correct RF library is used on the target device
# Set to 0x0 to disable the check, if radio features are going to be used the correct version needs to be set here and included with the update package
SD_VERSION=0x0

# Create an update package signed with the private key
nrfutil pkg generate --hw-version $HW_VERSION --application-version $APP_VERSION --application $APP_HEX_FILE --sd-req $SD_VERSION --key-file signing-key/dfu_private.key $OUTPUT_FILE
