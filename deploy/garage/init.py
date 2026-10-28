# SPDX-License-Identifier: AGPL-3.0-or-later
"""Prepare a fresh single-node Garage for notenwart through its admin API (v2).

Idempotent: assigns the node a role, creates the bucket, imports the access key and grants it the bucket.
Settings come from the environment: GARAGE_ADMIN_URL, GARAGE_ADMIN_TOKEN, GARAGE_CAPACITY (bytes, default
100 GB – Garage uses it only to balance nodes), S3_BUCKET, S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY.
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request

ADMIN = os.environ.get("GARAGE_ADMIN_URL", "http://garage:3903").rstrip("/")
TOKEN = os.environ["GARAGE_ADMIN_TOKEN"]
BUCKET = os.environ.get("S3_BUCKET", "notenwart")
KEY_ID = os.environ["S3_ACCESS_KEY_ID"]
SECRET = os.environ["S3_SECRET_ACCESS_KEY"]
CAPACITY = int(os.environ.get("GARAGE_CAPACITY", str(100 * 10**9)))


def call(endpoint, body=None, query=""):
    request = urllib.request.Request(
        f"{ADMIN}/v2/{endpoint}{query}",
        data=None if body is None else json.dumps(body).encode(),
        method="GET" if body is None else "POST",
        headers={"Authorization": f"Bearer {TOKEN}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            raw = response.read()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as error:
        raw = error.read()
        try:
            return error.code, json.loads(raw)
        except ValueError:
            return error.code, raw.decode(errors="replace")


def expect(result, what):
    status, body = result
    if status >= 300:
        sys.exit(f"garage-init: {what} failed ({status}): {body}")
    return body


def main():
    for _ in range(60):
        try:
            status, cluster = call("GetClusterStatus")
            if status == 200:
                break
        except OSError:
            pass
        time.sleep(1)
    else:
        sys.exit(f"garage-init: admin API at {ADMIN} not reachable")

    node = cluster["nodes"][0]["id"]
    layout = expect(call("GetClusterLayout"), "reading the layout")
    if not any(role["id"] == node for role in layout.get("roles", [])):
        expect(
            call("UpdateClusterLayout", {"roles": [{"id": node, "zone": "dc1", "capacity": CAPACITY, "tags": []}]}),
            "staging the layout",
        )
        expect(call("ApplyClusterLayout", {"version": layout["version"] + 1}), "applying the layout")
        print(f"garage-init: node {node[:16]} has a role")

    status, bucket = call("GetBucketInfo", query=f"?globalAlias={BUCKET}")
    if status != 200:
        bucket = expect(call("CreateBucket", {"globalAlias": BUCKET}), "creating the bucket")
        print(f"garage-init: bucket {BUCKET} created")

    status, _ = call("GetKeyInfo", query=f"?id={KEY_ID}")
    if status != 200:
        expect(
            call("ImportKey", {"accessKeyId": KEY_ID, "secretAccessKey": SECRET, "name": "notenwart"}),
            "importing the key",
        )
        print("garage-init: access key imported")

    expect(
        call(
            "AllowBucketKey",
            {"bucketId": bucket["id"], "accessKeyId": KEY_ID, "permissions": {"read": True, "write": True, "owner": True}},
        ),
        "granting the key",
    )
    print("garage-init: ready")


if __name__ == "__main__":
    main()
