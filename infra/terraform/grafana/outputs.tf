output "alert_rules_url" {
  description = "作ったアラートルールの一覧 (Grafana の Alerting)"
  value       = "${var.grafana_url}/alerting/list"
}

output "contact_point_name" {
  description = "通知先の名前 (Grafana の Alerting → Contact points に出る)"
  value       = grafana_contact_point.discord.name
}

output "logs_explore_hint" {
  description = "ログを検索するときの入り口 (Explore で Loki を選び、下のセレクタから始める)"
  value       = "{env=\"${var.log_gap_environment}\"} / {service=\"api\", level=\"ERROR\"}"
}
