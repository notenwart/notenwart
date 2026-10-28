#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prints fresh secrets for deploy/compose.yaml:  deploy/generate-secrets.sh > deploy/.env
set -eu
hex() { od -An -tx1 -N"$1" /dev/urandom | tr -d ' \n'; }
echo "POSTGRES_PASSWORD=$(hex 24)"
echo "GARAGE_RPC_SECRET=$(hex 32)"
echo "GARAGE_ADMIN_TOKEN=$(hex 32)"
echo "S3_ACCESS_KEY_ID=GK$(hex 12)"
echo "S3_SECRET_ACCESS_KEY=$(hex 32)"
