#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Restores an archive from deploy/backup.sh into a running deploy/compose.yaml installation. REPLACES the current
# data: the database is restored with --clean, PDFs not in the backup are deleted. From the repository root:
#   deploy/restore.sh backups/notenwart-….tar.gz
set -eu
cd "$(dirname "$0")/.."
compose() { docker compose -f deploy/compose.yaml "$@"; }
# Read only what is needed: .env may hold values a shell cannot source (e.g. MAIL_FROM=notenwart <…>).
env_value() { sed -n "s/^$1=//p" deploy/.env | tail -1; }
S3_ACCESS_KEY_ID=$(env_value S3_ACCESS_KEY_ID)
S3_SECRET_ACCESS_KEY=$(env_value S3_SECRET_ACCESS_KEY)

archive=${1:?usage: deploy/restore.sh <archive.tar.gz>}
[ -f "$archive" ] || { echo "no such file: $archive" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -C "$work" -xzf "$archive"
[ -f "$work/database.dump" ] || { echo "not a notenwart backup: $archive" >&2; exit 1; }

compose stop notenwart
compose exec -T db pg_restore -U notenwart -d notenwart --clean --if-exists --no-owner < "$work/database.dump"
network=$(docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' "$(compose ps -q garage)")
mkdir -p "$work/objects"
docker run --rm --network "$network" -v "$work:/backup" \
  -e AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY_ID" -e AWS_SECRET_ACCESS_KEY="$S3_SECRET_ACCESS_KEY" -e AWS_DEFAULT_REGION=garage \
  amazon/aws-cli:2.27.0 --endpoint-url http://garage:3900 s3 sync --delete --only-show-errors /backup/objects s3://notenwart
compose start notenwart
echo "restored $archive"
