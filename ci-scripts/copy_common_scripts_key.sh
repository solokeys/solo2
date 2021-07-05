#!/bin/bash
set -euo pipefail

echo "-----BEGIN OPENSSH PRIVATE KEY-----" > ~/.ssh/common_scripts_key
echo "$COMMON_SCRIPTS_KEY" >> ~/.ssh/common_scripts_key
echo "-----END OPENSSH PRIVATE KEY-----" >> ~/.ssh/common_scripts_key
chmod 600 ~/.ssh/common_scripts_key