#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Backs up a deploy/compose.yaml installation: the database (pg_dump) and every stored PDF (S3 bucket), into one
# archive. notenwart keeps running. From the repository root:
#   deploy/backup.sh [target directory]          (default: ./backups)
# Restore with deploy/restore.sh. Keep backups off the server, too.
set -eu
cd "$(dirname "$0")/.."
compose() { docker compose -f deploy/compose.yaml "$@"; }
# Read only what is needed: .env may hold values a shell cannot source (e.g. MAIL_FROM=notenwart <…>).
env_value() { sed -n "s/^$1=//p" deploy/.env | tail -1; }
S3_ACCESS_KEY_ID=$(env_value S3_ACCESS_KEY_ID)
S3_SECRET_ACCESS_KEY=$(env_value S3_SECRET_ACCESS_KEY)

target=${1:-backups}
stamp=$(date -u +%Y-%m-%dT%H%M%SZ)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

compose exec -T db pg_dump -U notenwart -Fc notenwart > "$work/database.dump"
network=$(docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' "$(compose ps -q garage)")
docker run --rm --network "$network" -v "$work:/backup" \
  -e AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY_ID" -e AWS_SECRET_ACCESS_KEY="$S3_SECRET_ACCESS_KEY" -e AWS_DEFAULT_REGION=garage \
  amazon/aws-cli:2.27.0 --endpoint-url http://garage:3900 s3 sync --only-show-errors s3://notenwart /backup/objects

mkdir -p "$target"
archive="$target/notenwart-$stamp.tar.gz"
tar -C "$work" -czf "$archive" .
echo "$archive"
