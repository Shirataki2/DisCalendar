variable "account_id" {
  description = "Cloudflare のアカウント ID (ダッシュボードの URL か R2 のエンドポイントに出る 32 桁)。terraform.tfvars で渡す"
  type        = string

  validation {
    condition     = can(regex("^[0-9a-f]{32}$", var.account_id))
    error_message = "account_id は 32 桁の 16 進数 (Cloudflare のアカウント ID)。"
  }
}

variable "backup_bucket_name" {
  description = "DB のバックアップ (pg_dump -Fc) を置く R2 バケットの名前"
  type        = string
  default     = "discalendar-backups"
}

variable "backup_bucket_location" {
  description = "バケットの配置ヒント。本番ホストが日本にあるので既定は apac (作成時の一度だけ効く)"
  type        = string
  default     = "apac"

  validation {
    condition     = contains(["apac", "eeur", "enam", "weur", "wnam", "oc"], var.backup_bucket_location)
    error_message = "backup_bucket_location は apac / eeur / enam / weur / wnam / oc のいずれか。"
  }
}

variable "backup_retention_days" {
  description = "バックアップを保持する日数。これを過ぎたオブジェクトはライフサイクルルールで自動削除される"
  type        = number
  default     = 30

  validation {
    condition     = var.backup_retention_days >= 1
    error_message = "backup_retention_days は 1 以上。"
  }
}

variable "backup_lock_days" {
  description = <<-EOT
    直近のバックアップを削除・上書きできなくする日数 (バケットロック)。
    R2 の API トークンは「削除だけを外す」粒度を持たないため、アップロード用トークンが漏れても
    この期間のバックアップは消せないようにするための保険。保持日数より短くすること
    (ロックはライフサイクルより優先されるので、同じ値にすると自動削除が効かなくなる)。
  EOT
  type        = number
  default     = 7

  validation {
    condition     = var.backup_lock_days >= 1 && var.backup_lock_days < var.backup_retention_days
    error_message = "backup_lock_days は 1 以上かつ backup_retention_days 未満。"
  }
}

variable "attachment_buckets" {
  description = "添付用バケット名と許可するWebオリジン。環境ごとに分ける。空なら作成しない"
  type        = map(list(string))
  default     = {}
  validation {
    condition = alltrue([
      for origins in values(var.attachment_buckets) : length(origins) > 0 && alltrue([
        for origin in origins : can(regex("^https://[^/*]+$|^http://(localhost|127\\.0\\.0\\.1)(:[0-9]+)?$", origin))
      ])
    ])
    error_message = "CORSは具体的なHTTPSオリジン（ローカル開発のみHTTP）を1つ以上指定してください。"
  }
  validation {
    condition     = !contains(keys(var.attachment_buckets), var.backup_bucket_name)
    error_message = "添付用バケットをバックアップ用バケットから分離してください。"
  }
}
