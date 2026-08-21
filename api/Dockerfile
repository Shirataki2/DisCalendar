# ビルドコンテキストはリポジトリルート (Cargo workspace):
#   docker build -f api/Dockerfile -t discalendar-api .
# DB 接続なしでビルドするため sqlx はオフラインモード (.sqlx/ を参照)。
# クエリを変更したら `cargo sqlx prepare` で .sqlx/ を更新してコミットすること。

FROM rust:1.98-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY api ./api
ENV SQLX_OFFLINE=true
RUN cargo build --release -p discalendar-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/discalendar-api /usr/local/bin/discalendar-api
ENV HOST=0.0.0.0 PORT=8080
EXPOSE 8080
USER nobody
CMD ["discalendar-api"]
