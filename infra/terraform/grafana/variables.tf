variable "grafana_url" {
  description = "Grafana Cloud のスタックの Grafana URL (https://<stack>.grafana.net)。認証トークンは環境変数 GRAFANA_AUTH で渡す"
  type        = string

  validation {
    condition     = can(regex("^https://[^/]+/?$", var.grafana_url))
    error_message = "grafana_url は https://<stack>.grafana.net の形 (パスは付けない)。"
  }
}

variable "loki_datasource_name" {
  description = <<-EOT
    ログを検索する Loki データソースの名前。Grafana Cloud のスタックには最初から
    grafanacloud-<組織のスラッグ>-logs という名前で用意されているので、その名前を書く
    (Grafana の Connections → Data sources で確認できる)。
  EOT
  type        = string
}

variable "discord_webhook_url" {
  description = <<-EOT
    アラートを流す Discord チャンネルの Webhook URL (チャンネルの設定 → 連携サービス → ウェブフック)。
    秘密情報なので tfvars には書かず、環境変数 TF_VAR_discord_webhook_url で渡す。
  EOT
  type        = string
  sensitive   = true

  validation {
    condition     = can(regex("^https://discord\\.com/api/webhooks/", var.discord_webhook_url))
    error_message = "discord_webhook_url は https://discord.com/api/webhooks/... の形。"
  }
}

variable "alert_folder_title" {
  description = "アラートルールを入れる Grafana のフォルダ名"
  type        = string
  default     = "DisCalendar"
}

variable "error_threshold" {
  description = <<-EOT
    直近 5 分の ERROR 件数がこれを超えたら通知する (env / service ごとに数える)。
    平常時は 0 件なので、一時的な失敗で鳴らないぶんだけ余裕を持たせた値にする。
  EOT
  type        = number
  default     = 10

  validation {
    condition     = var.error_threshold >= 1
    error_message = "error_threshold は 1 以上。"
  }
}

variable "log_gap_environment" {
  description = <<-EOT
    ログの途絶を見張る環境 (ラベル env の値)。この環境のログが log_gap_minutes のあいだ
    1 行も届かなければ通知する (ホストか alloy が落ちている、トークンが失効した、など)。
  EOT
  type        = string
  default     = "production"
}

variable "log_gap_minutes" {
  description = "ログが届かない状態がこの分数続いたら通知する"
  type        = number
  default     = 15

  validation {
    condition     = var.log_gap_minutes >= 5
    error_message = "log_gap_minutes は 5 以上 (短すぎるとデプロイ中の再起動で鳴る)。"
  }
}
