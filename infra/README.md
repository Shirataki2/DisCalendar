# infra

SaaS 側の設定 (Terraform) と、ホストで動かす運用スクリプト・エージェントの設定を置く。

| ディレクトリ | 中身 |
| --- | --- |
| [`terraform/`](terraform/) | Cloudflare の設定。今は DB バックアップ用の R2 バケットだけ (#102) |
| [`terraform/grafana/`](terraform/grafana/) | Grafana Cloud のログベースのアラートと Discord への通知 (#104) |
| [`backup/`](backup/) | 本番 / staging ホストで日次に動かす DB バックアップ (pg_dump → R2) |
| [`alloy/`](alloy/) | コンテナログを Grafana Cloud (Loki) に送る Grafana Alloy の設定 (#104) |

**秘密情報 (API トークン・アクセスキー・Webhook URL) はこの下に置かない**。値は各サービスのダッシュボードで発行して、
ローカルの環境変数かホストの `.env` / `/etc/discalendar/<環境名>.env` にだけ入れる。`terraform.tfvars` と `backend.hcl` は
git 管理外 (`terraform/.gitignore`) で、リポジトリにあるのは `*.example` だけ。

## Terraform

ダッシュボード任せにせず、リポジトリで diff を見られるようにするための下地。ルートモジュールは 2 つあり、
provider も認証情報も state も別になっている。**どちらも同じ手順 (`init -backend-config=backend.hcl` → `plan` → `apply`) で動かす**。

| ディレクトリ | provider | state のキー | 認証 |
| --- | --- | --- | --- |
| `terraform/` | cloudflare 5 系 | `discalendar/terraform.tfstate` | 環境変数 `CLOUDFLARE_API_TOKEN` |
| `terraform/grafana/` | grafana 4 系 | `discalendar/grafana.tfstate` | 環境変数 `GRAFANA_AUTH` |

provider はリソースの形が版によって動くのでバージョンを固定してある (`versions.tf`)。

## Cloudflare (`terraform/`)

### 管理しているもの / していないもの

- 管理している: DB バックアップ用の R2 バケット、そのライフサイクルルール (30 日で自動削除)、バケットロック (直近 7 日は削除・上書き不可)
- 管理していない: DNS / Cloudflare Tunnel / Redirect Rule (先に作ってあるもの。必要になったら `cf-terraforming` で取り込む)、
  R2 の API トークン (発行すると state に平文で載るため手で作る)

### 準備 (最初の 1 回)

1. **Terraform** (1.11 以上) を入れる。macOS なら `brew install terraform`
2. **state を置くバケットを手で作る**。Terraform 自身の state を置く場所なので Terraform では作れない
   (Cloudflare ダッシュボード → R2 → Create bucket → `discalendar-tfstate`、Location は `apac`)
3. **state 用のアクセスキーを作る**。R2 → API → Manage API tokens → Create API token
   - Permissions: **Object Read & Write**
   - Specify bucket: `discalendar-tfstate` だけ
   - 作成後に出る Access Key ID / Secret Access Key を控える (S3 互換 API 用の値。後から見られない)
4. **Cloudflare の API トークンを作る** (Terraform が R2 バケットを操作するためのもの。上のアクセスキーとは別)。
   My Profile → API Tokens → Create Token → Custom token
   - Permissions: **Account → Workers R2 Storage → Edit**
   - Account Resources: 対象のアカウントだけ
5. 設定ファイルを作る (どちらも git 管理外):

   ```sh
   cd infra/terraform
   cp terraform.tfvars.example terraform.tfvars   # account_id を書く
   cp backend.hcl.example backend.hcl             # state バケット名とエンドポイントを書く
   ```

### plan / apply

秘密は環境変数で渡す (シェルの履歴に残さないよう、`.env` を読むか `read -s` を使う):

```sh
cd infra/terraform

export CLOUDFLARE_API_TOKEN=...     # 手順 4 のトークン (Terraform が Cloudflare API を叩く)
export AWS_ACCESS_KEY_ID=...        # 手順 3 のアクセスキー (state の読み書き)
export AWS_SECRET_ACCESS_KEY=...

terraform init -backend-config=backend.hcl
terraform plan
terraform apply
```

- `terraform init` は最初の 1 回と、provider のバージョンを上げたときに実行する
- provider を上げるときは `versions.tf` の `version` を書き換えて
  `terraform init -upgrade -backend-config=backend.hcl` → `terraform plan` で差分を確認する。
  `.terraform.lock.hcl` は darwin_arm64 / linux_amd64 / linux_arm64 のハッシュを入れてあるので、
  更新は `terraform providers lock -platform=darwin_arm64 -platform=linux_amd64 -platform=linux_arm64`
- CI (`.github/workflows/ci.yml` の `infra` ジョブ) は `terraform fmt -check -recursive` と、ルートモジュールごとの
  `validate` だけを実行する。認証情報を GitHub 側に置かないので、`plan` / `apply` は手元から行う

## ログ集約とアラート (Grafana Cloud)

コンテナログはホストで 10MB × 3 世代までしか残らない (`compose.yaml` の `x-logging`) ので、
それより前を追えるように Grafana Cloud の Loki へ送る。Sentry (#17) との分担は次のとおり。

- **Sentry**: 例外のグルーピング・スタックトレース・ソースマップ。通知はメールだけ (無料プランは Discord 連携が使えない)
- **Grafana Cloud (ここ)**: ログの横断検索と、ログベースのアラートの Discord への通知

送っているのは compose のコンテナログ (web / api / bot / db / cloudflared)。ホスト側の systemd で動く
DB バックアップは journal に出るだけで、ここには乗らない (必要になったら `loki.source.journal` を足す)。

### 何にどんなラベルが付くか

`infra/alloy/config.alloy` が付けるラベルは 3 つだけ。ストリームが増えすぎないよう、
コンテナ ID や名前のように再作成で変わるものはラベルにしていない。

| ラベル | 値 | 付く範囲 |
| --- | --- | --- |
| `env` | `production` / `staging` (`.env` の `SENTRY_ENVIRONMENT`) | 全サービス |
| `service` | compose のサービス名 (`web` / `api` / `bot` / `db` / `cloudflared`) | 全サービス |
| `level` | `ERROR` / `WARN` / `INFO` など | api / bot だけ (`LOG_FORMAT=json` の JSON ログから起こす) |

web と db はテキストログなので `level` が付かない。本文の全文検索 (`|= "error"`) で追う。

### Grafana Cloud の準備 (最初の 1 回)

1. [Grafana Cloud](https://grafana.com/) にサインアップしてスタックを作る (無料枠: ログ 50GB/月・保持 14 日)
2. スタックの **Loki** の詳細 (Send Logs) に出る **push URL** と **User** を控える
3. **アクセスポリシートークン**を作る (Access Policies → Create access policy)。スコープは `logs:write` だけ、
   有効期限は無期限でよい。作成後に一度だけ表示される値を控える
4. Terraform 用に **サービスアカウントとトークン**を作る (Grafana の Administration → Users and access →
   Service accounts)。ロールは **Admin** (アラートルールと通知先を作るため)
5. アラートを流す Discord チャンネルの **Webhook URL** を作る (チャンネルの設定 → 連携サービス → ウェブフック)

### ホスト側の設定 (ログを送る)

ホストの `.env` (compose と同じディレクトリ) に手順 2・3 の値を入れて、profile を足す:

```sh
GRAFANA_CLOUD_LOKI_URL=https://logs-prod-030.grafana.net/loki/api/v1/push
GRAFANA_CLOUD_LOKI_USER=1234567
GRAFANA_CLOUD_LOKI_TOKEN=glc_...
COMPOSE_PROFILES=bot,tunnel,logging
```

```sh
docker compose up -d          # alloy が起動する
docker compose logs alloy     # 送信できないときはここにエラーが出る
```

設定ファイル (`infra/alloy/config.alloy`) はデプロイのたびに compose.yaml と一緒に配られ、
`.github/scripts/deploy.sh` が `docker compose restart alloy` で読み直させる
(bind mount なので、up -d だけでは新しい設定が反映されない)。手で置くときは compose.yaml から見て
同じ相対パス (`<compose のディレクトリ>/infra/alloy/config.alloy`) に置くこと。

### アラート (`terraform/grafana/`)

ルールと通知先は Terraform で管理する。作られるのは 2 本:

- **ERROR ログの急増**: `{service=~"api|bot", level="ERROR"}` を 5 分窓で数え、`error_threshold` (既定 10) を超えたら通知
- **ログの途絶**: 本番のログが `log_gap_minutes` (既定 15) 分 1 行も届かなければ通知 (ホストか alloy が落ちている)

```sh
cd infra/terraform/grafana
cp terraform.tfvars.example terraform.tfvars   # スタックの URL と Loki データソース名を書く
cp backend.hcl.example backend.hcl             # state バケット (Cloudflare 側と同じ) を書く

export GRAFANA_AUTH=...                        # 手順 4 のサービスアカウントトークン
export TF_VAR_discord_webhook_url=...          # 手順 5 の Webhook URL
export AWS_ACCESS_KEY_ID=...                   # state の読み書き (Cloudflare 側と同じキー)
export AWS_SECRET_ACCESS_KEY=...

terraform init -backend-config=backend.hcl
terraform plan
terraform apply
```

- `loki_datasource_name` は Grafana の Connections → Data sources に出ている名前
  (`grafanacloud-<組織のスラッグ>-logs`)。UID は Terraform が名前から引く
- 通知先は root の通知ポリシーではなくルール側 (`notification_settings`) で指定してあるので、
  Grafana Cloud が既定で持っている通知の設定には触らない
- 実際に鳴るか試すには、Grafana の Alerting → Contact points で **Test** を押す

### ログを見る

Grafana の **Explore** で Loki を選び、次のようなクエリを使う。

```logql
{env="production"}                                  # 全サービス
{env="production", service="api", level="ERROR"}    # api のエラーだけ
{env="production", service="web"} |= "Error"        # web はテキストログなので全文検索
{env="production"} | json | line_format "{{.fields_message}}"   # JSON を読みやすく整形する
```

### リクエスト単位のエラー率とレイテンシ

api はリクエストが終わるたびに `request completed` の 1 行を出す (#110)。`LOG_FORMAT=json` (compose の既定)
ならこの形で、`| json` を通すと `fields.status` が `fields_status` のように読める。

```json
{
  "timestamp": "2026-08-30T12:34:56.789012Z",
  "level": "INFO",
  "fields": {
    "message": "request completed",
    "status": 200,
    "method": "GET",
    "route": "/guilds/{guild_id}/events",
    "duration_ms": 12.345
  },
  "target": "discalendar_api::logging"
}
```

この行には root span のフィールド (`span` / `spans` に `request_id`・`http.target`・`http.user_agent` など) も
一緒に載る。ERROR 行と同じ `request_id` なので、5xx の完了ログとその原因のログを突き合わせられる。

- `route` は `/guilds/{guild_id}/events` のようなルートのパターンで、予定 ID などの実際の値は入らない
  (どのルートが遅いかを集計するため)。登録されていないパスへの 404 は `unmatched` になる
- `status` と `duration_ms` は JSON の数値。`duration_ms` はミリ秒 (小数以下 3 桁) で、
  応答のヘッダを組み立て終えるまでを測る (gzip 圧縮とボディの送出は含まない)
- 5xx のときは `error` にエラーの内容が入る。原因の詳細は同じ `request_id` の ERROR 行と Sentry 側を見る
- 10 秒おきに叩かれる `/healthz` は DEBUG なので本番のログには出ない
  (見たいときはホストの `.env` で `API_RUST_LOG=info,discalendar_api=debug,sqlx=warn`)

**ラベルは増やしていない**ので、集計は本文を `| json` で読む。ストリームを増やさないため、
`fields_route` などをラベルに起こすことはしない。

```logql
# リクエスト完了ログだけを見る (行フィルタで絞ってから json を通すと軽い)
{env="production", service="api"} |= "request completed" | json

# 5 分あたりのエラー率 (5xx の割合)
sum(count_over_time({env="production", service="api"} |= "request completed" | json | fields_status >= 500 [5m]))
  / sum(count_over_time({env="production", service="api"} |= "request completed" [5m]))

# ルート別のリクエスト数 (5 分)
sum by (fields_route) (count_over_time({env="production", service="api"} |= "request completed" | json [5m]))

# 遅いエンドポイント (5 分の p95 レイテンシ 上位 5 件、ミリ秒)
topk(5, quantile_over_time(0.95,
  {env="production", service="api"} |= "request completed" | json | unwrap fields_duration_ms [5m]
) by (fields_route))

# 直近の 5xx を新しい順に読む
{env="production", service="api"} |= "request completed" | json | fields_status >= 500
```

## DB のバックアップ (pg_dump → R2)

本番 (と必要なら staging) の compose の `db` から `pg_dump -Fc` したダンプを日次で R2 に上げる。
世代管理は R2 のライフサイクルルールに任せ、スクリプトは古いダンプを消さない。

- **消す権限を渡さない**: R2 の API トークンは「削除だけ外す」粒度を持たない (Object Read & Write に削除が含まれる) ので、
  代わりにバケットロックで直近 7 日ぶんを削除・上書き不可にしてある。ホストが乗っ取られてキーが漏れても、
  直近のバックアップは消せない (ロックはライフサイクルより優先されるため、保持日数 30 日 > ロック 7 日 にしてある)
- **保存期間**: 30 日 (`backup_retention_days`)。日次 1 世代なので常時 30 ファイル程度。R2 の無料枠は 10GB/月

### ホストへの設置

ホストで docker が動いていることが前提 (aws-cli はコンテナで動かすのでホストに入れなくてよい)。

```sh
# 1. スクリプトを置く (リポジトリの infra/backup/ から)
sudo install -d /opt/discalendar-backup
sudo install -m 755 backup-db.sh r2.sh /opt/discalendar-backup/

# 2. 設定を置く (環境ごとに 1 つ。production / staging がインスタンス名になる)
sudo install -d -m 755 /etc/discalendar
sudo install -m 600 backup.env.example /etc/discalendar/production.env
sudo vi /etc/discalendar/production.env   # アカウント ID・バケット・アクセスキーを書く

# 3. systemd の unit を置いて有効にする
sudo install -m 644 'discalendar-backup@.service' 'discalendar-backup@.timer' /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now discalendar-backup@production.timer

# 4. 手で 1 回動かして確認する
sudo systemctl start discalendar-backup@production.service
journalctl -u discalendar-backup@production.service -n 50 --no-pager
systemctl list-timers 'discalendar-backup@*'
```

バックアップ用のアクセスキーは、Terraform で作ったバケットに対して発行する
(R2 → API → Manage API tokens → **Object Read & Write**、Specify bucket に `discalendar-backups` だけ)。

失敗すると service が `failed` になり、内容は `journalctl` に残る。**失敗しても通知は飛ばない**ので、
`systemctl list-timers` と R2 のオブジェクト一覧をときどき見る (systemd の journal はコンテナログではないので、
上のログ集約にも乗らない。ここも通知したくなったら alloy に `loki.source.journal` を足す)。

### 中身を見る / 落とす

`r2.sh` は `backup.env` を読んで aws-cli をコンテナで動かすラッパ。**カレントディレクトリをコンテナにマウントする**ので、
ローカル側のパスは相対パスで渡す。

```sh
cd /opt/discalendar-backup

# 一覧 (新しいものが下)
sudo BACKUP_ENV_FILE=/etc/discalendar/production.env ./r2.sh s3 ls "s3://discalendar-backups/production/"

# 落とす (カレントディレクトリに置かれる)
cd /var/tmp
sudo BACKUP_ENV_FILE=/etc/discalendar/production.env /opt/discalendar-backup/r2.sh \
  s3 cp "s3://discalendar-backups/production/discalendar-20260827T190000Z.dump" .
```

### 復元

手順はルート [README.md](../README.md) の「DB のバックアップと復元」を参照。
R2 から落としたダンプを、**空の DB に対して** `pg_restore` する (api を起動する前に行う)。
