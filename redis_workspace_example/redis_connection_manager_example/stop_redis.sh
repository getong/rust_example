#!/bin/sh

docker stop redis-raft-3 redis-raft-2 redis-raft-1
docker rm redis-raft-3 redis-raft-2 redis-raft-1
