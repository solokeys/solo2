#!/usr/bin/env bash

# Generate a private key
nrfutil keys generate dfu_private.key

# Generate C source file containing the public key
nrfutil keys display --key pk --format code dfu_private.key --out_file dfu_public_key.c
