#!/bin/sh

# copy from https://agones.dev/site/docs/installation/install-agones/yaml/

kubectl create namespace agones-system
kubectl apply --server-side -f https://raw.githubusercontent.com/googleforgames/agones/release-1.59.0/install/yaml/install.yaml
