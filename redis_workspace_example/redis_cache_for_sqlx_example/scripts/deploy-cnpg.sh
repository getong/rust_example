#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NAMESPACE="${NAMESPACE:-redis-cache-example}"
command -v kubectl >/dev/null || { printf 'kubectl is required.\n' >&2; exit 1; }
printf 'Kubernetes context: %s\n' "$(kubectl config current-context)"
if ! kubectl get crd clusters.postgresql.cnpg.io >/dev/null; then
  printf 'Install the CloudNativePG operator in this context before deploying.\n' >&2
  exit 1
fi
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
kubectl apply --dry-run=server -n "$NAMESPACE" -f "$PROJECT_ROOT/k8s/cloudnative-pg.yaml"
kubectl apply -n "$NAMESPACE" -f "$PROJECT_ROOT/k8s/cloudnative-pg.yaml"
kubectl wait -n "$NAMESPACE" --for=condition=Ready cluster/app-db --timeout=600s
printf 'Write endpoint: app-db-rw.%s.svc.cluster.local:5432\n' "$NAMESPACE"
printf 'Application credentials: Secret app-db-app in namespace %s\n' "$NAMESPACE"
