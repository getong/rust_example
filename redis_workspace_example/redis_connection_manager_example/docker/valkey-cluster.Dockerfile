FROM valkey/valkey:9.0.1-alpine3.23

# Entrypoint builds a per-node cluster config and launches valkey-server.
COPY docker/valkey-cluster-entrypoint.sh /usr/local/bin/valkey-cluster-entrypoint.sh
RUN chmod +x /usr/local/bin/valkey-cluster-entrypoint.sh

EXPOSE 6379 16379

ENTRYPOINT ["/usr/local/bin/valkey-cluster-entrypoint.sh"]
