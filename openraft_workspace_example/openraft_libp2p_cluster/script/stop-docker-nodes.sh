#!/usr/bin/env bash
# Stop and remove every container started by run-100docker.sh (all containers
# labeled olpc-cluster=demo, including the redis/valkey container), their
# anonymous data volumes, and the olpc-net network.
set -euo pipefail

LABEL="olpc-cluster=demo"
NET="${NET:-olpc-net}"

ids="$(docker ps -aq --filter "label=$LABEL")"
if [[ -n "$ids" ]]; then
	echo "Removing $(wc -l <<<"$ids" | tr -d ' ') container(s) labeled $LABEL..."
	# shellcheck disable=SC2086
	docker rm -f -v $ids >/dev/null
else
	echo "No containers labeled $LABEL."
fi

if docker network inspect "$NET" >/dev/null 2>&1; then
	docker network rm "$NET" >/dev/null
	echo "Removed network $NET."
fi

echo "Done."
