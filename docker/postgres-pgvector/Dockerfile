# LOCAL-EMBEDDING-001 PostgreSQL base.  Docker Hub's pgvector image is not a deployment authority:
# build the extension from an auditable pgvector commit on the already-pinned PostgreSQL 16.14 base.
FROM postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8 AS build

ARG PGVECTOR_COMMIT=2627c5ff775ae6d7aef0c430121ccf857842d2f2
ARG POSTGRES_SERVER_DEV_VERSION=16.14-1.pgdg12+1

RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential ca-certificates git "postgresql-server-dev-16=${POSTGRES_SERVER_DEV_VERSION}" \
 && test "$(pg_config --version)" = "PostgreSQL 16.14 (Debian 16.14-1.pgdg12+1)" \
 && rm -rf /var/lib/apt/lists/*
RUN git clone https://github.com/pgvector/pgvector.git /tmp/pgvector \
 && git -C /tmp/pgvector checkout --detach "$PGVECTOR_COMMIT" \
 && test "$(git -C /tmp/pgvector rev-parse HEAD)" = "$PGVECTOR_COMMIT" \
 && make -C /tmp/pgvector \
 && make -C /tmp/pgvector install

FROM postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8

COPY --from=build /usr/lib/postgresql/16/lib/vector.so /usr/lib/postgresql/16/lib/vector.so
COPY --from=build /usr/share/postgresql/16/extension/vector.control /usr/share/postgresql/16/extension/vector.control
COPY --from=build /usr/share/postgresql/16/extension/vector--*.sql /usr/share/postgresql/16/extension/
