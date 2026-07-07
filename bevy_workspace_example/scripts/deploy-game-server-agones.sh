#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

namespace="${GAME_SERVER_NAMESPACE:-default}"
fleet="${GAME_SERVER_FLEET:-bevy-game-server}"
timeout_seconds="${GAME_SERVER_WAIT_SECONDS:-180}"

command -v kubectl >/dev/null 2>&1 || {
  echo "kubectl is required" >&2
  exit 1
}

"${repo_root}/scripts/ensure-agones.sh"

echo "Applying Agones game server manifests"
kubectl apply -f deploy/agones

echo "Waiting for fleet/${fleet} in namespace ${namespace}"
deadline=$((SECONDS + timeout_seconds))
while true; do
  desired="$(kubectl get "fleet/${fleet}" -n "${namespace}" -o jsonpath='{.spec.replicas}' 2>/dev/null || true)"
  current="$(kubectl get "fleet/${fleet}" -n "${namespace}" -o jsonpath='{.status.replicas}' 2>/dev/null || true)"
  ready="$(kubectl get "fleet/${fleet}" -n "${namespace}" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || true)"

  if [[ -n "${desired}" && "${current:-0}" == "${desired}" && "${ready:-0}" == "${desired}" ]]; then
    break
  fi

  if (( SECONDS >= deadline )); then
    echo "timed out waiting for fleet/${fleet}: desired=${desired:-?} current=${current:-0} ready=${ready:-0}" >&2
    kubectl get fleet,gameserver,pods -n "${namespace}" -o wide
    exit 1
  fi

  sleep 2
done

kubectl get fleet,gameserver,pods -n "${namespace}" -o wide
kubectl get svc/bevy-game-server-local endpoints/bevy-game-server-local -n "${namespace}" -o wide

echo "Local client endpoint: 127.0.0.1:30600"
