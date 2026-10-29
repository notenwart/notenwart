# SPDX-License-Identifier: AGPL-3.0-or-later
# One image: the Rust server with the web app embedded.

FROM node:26-bookworm-slim AS web
RUN corepack enable
WORKDIR /src
COPY api api
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml web/.npmrc web/
RUN cd web && pnpm install --frozen-lockfile
COPY web web
RUN cd web && pnpm build

FROM rust:1-bookworm AS server
WORKDIR /src
# rust-toolchain.toml pins the toolchain; rustup in the image fetches it.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .sqlx .sqlx
COPY crates crates
# Queries are checked against .sqlx (`cargo sqlx prepare --workspace -- --all-targets`), not a live database.
ENV SQLX_OFFLINE=true
COPY --from=web /src/web/dist web/dist
RUN cargo build --release --locked

FROM debian:bookworm-slim
# poppler-utils and tesseract read the text of sheet music (crates/notenwart-app/src/recognize.rs).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
        poppler-utils tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 notenwart \
    && install -d -o notenwart -g notenwart /var/lib/notenwart
COPY --from=server /src/target/release/notenwart /usr/local/bin/notenwart
USER notenwart
WORKDIR /var/lib/notenwart
# Used with STORAGE=fs (the default); with S3 the volume stays empty.
ENV STORAGE_PATH=/var/lib/notenwart/data
VOLUME /var/lib/notenwart
EXPOSE 8080
ENTRYPOINT ["notenwart"]
CMD ["serve"]
