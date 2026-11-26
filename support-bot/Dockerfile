# ビルドコンテキストはリポジトリルート (Cargo workspace):
#   docker build -f support-bot/Dockerfile -t discalendar-support-bot .
# DB 接続なしでビルドするため sqlx はオフラインモード (.sqlx/ を参照)。
# クエリを変更したら support-bot/ で `cargo sqlx prepare -- --all-targets` を実行して .sqlx/ を更新しコミットすること。

FROM rust:1.98-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
# workspace の members をすべて置かないと cargo が manifest を読めないので api / bot も入れる
COPY api ./api
COPY bot ./bot
COPY support-bot ./support-bot
COPY shared ./shared
# 更新の告知 (#430) で更新履歴の本文をビルド時に埋め込む (src/announce.rs の include_str!)
COPY web/src/content/changelog.mdx ./web/src/content/changelog.mdx
ENV SQLX_OFFLINE=true
RUN cargo build --release -p discalendar-support-bot --bin discalendar-support-bot

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/discalendar-support-bot /usr/local/bin/discalendar-support-bot
USER nobody
CMD ["discalendar-support-bot"]
