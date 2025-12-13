FROM redis:8.4-alpine

WORKDIR /app

# Copy base config and entrypoint.
COPY docker/redis-raft.conf /etc/redis/redis-raft.conf
COPY docker/entrypoint.sh /usr/local/bin/redis-raft-entrypoint.sh

RUN chmod +x /usr/local/bin/redis-raft-entrypoint.sh

EXPOSE 6379

ENTRYPOINT ["/usr/local/bin/redis-raft-entrypoint.sh"]
