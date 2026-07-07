#!/usr/bin/env bash
set -euo pipefail

version="${AGONES_VERSION:-1.59.0}"
namespace="${AGONES_NAMESPACE:-agones-system}"
timeout="${AGONES_WAIT_TIMEOUT:-300s}"
install_url="${AGONES_INSTALL_URL:-https://raw.githubusercontent.com/googleforgames/agones/release-${version}/install/yaml/install.yaml}"

command -v kubectl >/dev/null 2>&1 || {
  echo "kubectl is required" >&2
  exit 1
}

echo "kubectl context: $(kubectl config current-context)"

if kubectl get crd gameservers.agones.dev >/dev/null 2>&1; then
  echo "Agones CRDs are already installed"
else
  echo "Installing Agones ${version} from ${install_url}"
  kubectl create namespace "${namespace}" --dry-run=client -o yaml | kubectl apply -f -
  kubectl apply -f "${install_url}"
fi

echo "Waiting for Agones pods in namespace ${namespace}"
kubectl wait --for=condition=Ready pod --all -n "${namespace}" --timeout="${timeout}"
kubectl get pods -n "${namespace}" -o wide
