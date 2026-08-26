# syntax=docker/dockerfile:1
# ビルドコンテキストは web/:
#   docker build -t discalendar-web web
# next.config.ts の output: "standalone" で .next/standalone/ に実行に必要なファイルだけを出し、
# それを node:slim に載せる。pnpm の版は package.json の packageManager (corepack) に従う。

ARG NODE_VERSION=24

# rewrites (/local/api/* → Rust API) の宛先は next build 時に routes-manifest.json へ焼き込まれるため、
# 実行時の環境変数では変えられない。compose のサービス名 (api) を既定にしておき、
# 別のホスト名にしたいときは --build-arg API_URL=... でビルドし直す
ARG API_URL=http://api:8080

# OGP など絶対 URL が必要なメタ情報の基準 (src/lib/site.ts の SITE_URL)。これも next build 時に
# 焼き込まれるので、staging のように別ドメインで出すときは --build-arg NEXT_PUBLIC_SITE_URL=... でビルドし直す
ARG NEXT_PUBLIC_SITE_URL=https://discalendar.app

FROM node:${NODE_VERSION}-slim AS build
ARG API_URL
ARG NEXT_PUBLIC_SITE_URL
WORKDIR /app
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    NEXT_TELEMETRY_DISABLED=1
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY . .
# ビルド自体に本物の値は不要 (CI と同じダミー)。未設定だと Better Auth が警告を出す
RUN API_URL=${API_URL} \
    NEXT_PUBLIC_SITE_URL=${NEXT_PUBLIC_SITE_URL} \
    BETTER_AUTH_SECRET=build-dummy-secret-not-used-at-runtime \
    BETTER_AUTH_URL=http://localhost:3000 \
    pnpm build

FROM node:${NODE_VERSION}-slim AS runner
ARG API_URL
WORKDIR /app
ENV NODE_ENV=production \
    NEXT_TELEMETRY_DISABLED=1 \
    HOSTNAME=0.0.0.0 \
    PORT=3000 \
    # Server Component から API を直接呼ぶときの URL (src/lib/api/server.ts)。rewrites と同じ値にする
    API_URL=${API_URL} \
    # 起動時に Better Auth のテーブルを DATABASE_URL へ作成・更新する (src/instrumentation.ts)
    AUTO_MIGRATE=true
COPY --from=build --chown=node:node /app/.next/standalone ./
COPY --from=build --chown=node:node /app/.next/static ./.next/static
COPY --from=build --chown=node:node /app/public ./public
USER node
EXPOSE 3000
CMD ["node", "server.js"]
