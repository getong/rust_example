#!/bin/sh

## copy from https://surrealdb.com/docs/surrealdb/installation/running/docker

mkdir -p mydata

docker run --rm -d -p 9000:8000 \
  --user $(id -u) -v $(pwd)/mydata:/mydata \
  surrealdb/surrealdb:latest start --log debug \
  --allow-all --user root --pass root \
  rocksdb:/mydata/rocksdb