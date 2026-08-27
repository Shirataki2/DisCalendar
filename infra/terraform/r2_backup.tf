# 本番 DB の日次バックアップ (pg_dump -Fc) を置くバケット (#102)。
# アップロードするのはホスト側の systemd timer (infra/backup/)。世代管理はここのライフサイクルルールに任せる。

resource "cloudflare_r2_bucket" "backups" {
  account_id = var.account_id
  name       = var.backup_bucket_name
  location   = var.backup_bucket_location
  # 日次のダンプは復元時にすぐ落としたいので Standard のまま (InfrequentAccess は取り出しに課金がある)
  storage_class = "Standard"
}

resource "cloudflare_r2_bucket_lifecycle" "backups" {
  account_id  = var.account_id
  bucket_name = cloudflare_r2_bucket.backups.name

  rules = [
    # 世代管理。ホスト側でスクリプトが古いダンプを消す必要はない (消す権限も持たせない)
    {
      id         = "expire-old-backups"
      enabled    = true
      conditions = { prefix = "" }
      delete_objects_transition = {
        condition = {
          type    = "Age"
          max_age = var.backup_retention_days * 24 * 60 * 60
        }
      }
    },
    # アップロードが途中で失敗したときに残るマルチパートの断片を掃除する (残ると保存容量を食う)
    {
      id         = "abort-incomplete-multipart-uploads"
      enabled    = true
      conditions = { prefix = "" }
      abort_multipart_uploads_transition = {
        condition = {
          type    = "Age"
          max_age = 24 * 60 * 60
        }
      }
    },
  ]
}

# 直近のバックアップを消せなくする (ホストが乗っ取られてアップロード用トークンが漏れても過去分が残るように)。
# 解除にはバケット設定を編集できる権限が要り、アップロード用のトークン (Object Read & Write) では変更できない。
resource "cloudflare_r2_bucket_lock" "backups" {
  account_id  = var.account_id
  bucket_name = cloudflare_r2_bucket.backups.name

  rules = [
    {
      id      = "retain-recent-backups"
      enabled = true
      prefix  = ""
      condition = {
        type            = "Age"
        max_age_seconds = var.backup_lock_days * 24 * 60 * 60
      }
    },
  ]
}
