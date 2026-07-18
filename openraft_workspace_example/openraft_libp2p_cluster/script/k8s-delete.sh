#!/usr/bin/env bash
# Tear down everything k8s-run-100nodes.sh created: the whole olpc namespace
# (statefulsets, services, redis, secret, configmap) in one delete.
set -euo pipefail

NS=olpc

if ! kubectl get namespace "$NS" >/dev/null 2>&1; then
	echo "Namespace $NS does not exist; nothing to delete."
	exit 0
fi

echo "Deleting namespace $NS (all 100 pods, services, secret, configmap)..."
kubectl delete namespace "$NS" --wait="${WAIT:-true}"
echo "Done."
