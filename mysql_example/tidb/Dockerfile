FROM ubuntu:25.04

ENV DEBIAN_FRONTEND=noninteractive \
    TZ=Asia/Tokyo

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl gnupg2 ca-certificates lsb-release bash sudo ssh \
    mariadb-client tzdata \
 && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://tiup-mirrors.pingcap.com/install.sh | sh && \
    export PATH=/root/.tiup/bin:$PATH && \
    tiup update --self && \
    tiup update playground && \
    tiup install tidb:v7.1.1 tikv:v7.1.1 pd:v7.1.1 tiflash:v7.1.1

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

COPY sql/ /docker-entrypoint-initdb.d/

ENV TIDB_VERSION=v7.1.1 \
    TIDB_HOST=0.0.0.0 \
    TIDB_TAG=test-cluster \
    DB_HOST=127.0.0.1 \
    DB_PORT=4000 \
    DB_USER=root \
    DB_NAME=test

EXPOSE 4000 2379 3000

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
