# ビルドコンテキストはリポジトリルート (Cargo workspace):
#   docker build -f bot/Dockerfile -t discalendar-bot .
# DB 接続なしでビルドするため sqlx はオフラインモード (.sqlx/ を参照)。
# クエリを変更したら bot/ で `cargo sqlx prepare` を実行して .sqlx/ を更新しコミットすること。

FROM rust:1.98-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
# workspace の members をすべて置かないと cargo が manifest を読めないので api も入れる
COPY api ./api
COPY bot ./bot
ENV SQLX_OFFLINE=true
RUN cargo build --release -p discalendar-bot

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/discalendar-bot /usr/local/bin/discalendar-bot
# icon_updater タスクが相対パス "assets/" から日付入りアイコンを読むので、実行時の cwd (WORKDIR) に置く
COPY bot/assets ./assets
USER nobody
CMD ["discalendar-bot"]
