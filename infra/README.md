# infra

Cloudflare まわりの設定 (Terraform) と、ホストで動かす運用スクリプトを置く。

| ディレクトリ | 中身 |
| --- | --- |
| [`terraform/`](terraform/) | Cloudflare の設定。今は DB バックアップ用の R2 バケットだけ (#102) |
| [`backup/`](backup/) | 本番 / staging ホストで日次に動かす DB バックアップ (pg_dump → R2) |

**秘密情報 (API トークン・アクセスキー) はこの下に置かない**。値は Cloudflare のダッシュボードで発行して、
ローカルの環境変数かホストの `/etc/discalendar/<環境名>.env` にだけ入れる。`terraform.tfvars` と `backend.hcl` は
git 管理外 (`terraform/.gitignore`) で、リポジトリにあるのは `*.example` だけ。

## Terraform

Cloudflare の設定をダッシュボード任せにせず、リポジトリで diff を見られるようにするための下地。
provider は 5 系でリソースごとに粗さが残るのでバージョンを固定してある (`terraform/versions.tf`)。

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
- CI (`.github/workflows/ci.yml` の `terraform` ジョブ) は `terraform fmt -check` と `validate` だけを実行する。
  Cloudflare の認証情報を GitHub 側に置かないので、`plan` / `apply` は手元から行う

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
`systemctl list-timers` と R2 のオブジェクト一覧をときどき見る (ログの集約とアラートは #104)。

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
