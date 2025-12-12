FROM redis:8.4-alpine

WORKDIR /app

# Copy RedisRaft module and base config.
ARG RAFT_MODULE=redisraft.so
COPY ${RAFT_MODULE} /usr/lib/redisraft.so
COPY docker/redis-raft.conf /etc/redis/redis-raft.conf
COPY docker/entrypoint.sh /usr/local/bin/redis-raft-entrypoint.sh

RUN chmod +x /usr/local/bin/redis-raft-entrypoint.sh

EXPOSE 6379 5001

ENTRYPOINT ["/usr/local/bin/redis-raft-entrypoint.sh"]
