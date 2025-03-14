#!/bin/sh

## copy from https://surrealdb.com/docs/surrealdb/installation/running/docker

mkdir -p mydata

docker run --rm -d --name my_surrealdb -p 9000:8000 \
  --user $(id -u) -v $(pwd)/mydata:/mydata \
  surrealdb/surrealdb:v2 start --log debug \
  --allow-all --user root --pass root \
  rocksdb:/mydata/