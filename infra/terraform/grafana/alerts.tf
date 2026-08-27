# ログベースのアラートと、その通知先 (Discord) (#104)。
# ログを送っているのはホスト側の alloy (compose の logging profile、infra/alloy/config.alloy)。
# 検索できるラベルは env (production / staging)・service (compose のサービス名)・level (api / bot のみ)。

# スタックに最初からある Loki データソース (ログの検索先)。名前は組織ごとに違うので変数で渡す
data "grafana_data_source" "logs" {
  name = var.loki_datasource_name
}

resource "grafana_folder" "alerts" {
  title = var.alert_folder_title
}

# Sentry の無料プランは Discord 連携を持たない (メール通知のみ) ため、
# Discord への通知はこちらのログベースのアラートが担う (#104 のコメント参照)
resource "grafana_contact_point" "discord" {
  name = "discalendar-discord"

  discord {
    url = var.discord_webhook_url
    # 本文は Grafana の既定テンプレート (ルール名・ラベル・annotations が並ぶ) のまま使う
    use_discord_username = true
  }
}

locals {
  # 直近 5 分の ERROR を env / service ごとに数える。level ラベルは alloy が
  # api / bot の JSON ログ (LOG_FORMAT=json) から起こしている
  error_count_expr = "sum by (env, service) (count_over_time({service=~\"api|bot\", level=\"ERROR\"}[5m]))"

  # 監視対象の環境から届いたログの数。0 件 (= 結果なし) が続く状態を no_data_state で拾う
  log_gap_expr = "sum(count_over_time({env=\"${var.log_gap_environment}\"}[${var.log_gap_minutes}m]))"
}

resource "grafana_rule_group" "logs" {
  name             = "logs"
  folder_uid       = grafana_folder.alerts.uid
  interval_seconds = 60

  # 平常時の ERROR は 0 件なので、しきい値を超えたらすぐ知りたい (for は置かない)
  rule {
    name      = "ERROR ログの急増"
    condition = "B"
    for       = "0s"
    # ERROR が 1 件も無いと Loki は結果を返さず NoData になる。これは正常な状態なので鳴らさない
    no_data_state  = "OK"
    exec_err_state = "Error"

    data {
      ref_id = "A"
      relative_time_range {
        from = 600
        to   = 0
      }
      datasource_uid = data.grafana_data_source.logs.uid
      model = jsonencode({
        refId         = "A"
        datasource    = { type = "loki", uid = data.grafana_data_source.logs.uid }
        expr          = local.error_count_expr
        queryType     = "instant"
        intervalMs    = 1000
        maxDataPoints = 43200
      })
    }

    data {
      ref_id = "B"
      relative_time_range {
        from = 0
        to   = 0
      }
      # 式 (しきい値判定) は datasource_uid に -100 を置く決まりになっている
      datasource_uid = "-100"
      model = jsonencode({
        refId      = "B"
        type       = "threshold"
        datasource = { type = "__expr__", uid = "-100" }
        expression = "A"
        conditions = [{
          type      = "query"
          evaluator = { type = "gt", params = [var.error_threshold] }
          operator  = { type = "and" }
          query     = { params = ["A"] }
          reducer   = { type = "last", params = [] }
        }]
      })
    }

    annotations = {
      summary     = "{{ $labels.env }} の {{ $labels.service }} で ERROR が 5 分に ${var.error_threshold} 件を超えました"
      description = "Grafana の Explore で {service=\"{{ $labels.service }}\", env=\"{{ $labels.env }}\", level=\"ERROR\"} を見る。例外の詳細は Sentry 側 (#17)"
    }
    labels = { severity = "warning" }

    notification_settings {
      contact_point = grafana_contact_point.discord.name
      # サービスと環境ごとに別の通知にする (alertname と grafana_folder は指定が要る)
      group_by = ["alertname", "grafana_folder", "env", "service"]
    }
  }

  # ログが届かなくなったことに気づくためのルール。ホストか alloy が落ちた、トークンが失効した、
  # といったときに鳴る (バックアップの失敗も含め、ホストの異常はまずここに出る)
  rule {
    name      = "ログの途絶"
    condition = "B"
    for       = "0s"
    # 1 行も届いていない = NoData。これがまさに検知したい状態なので発報させる
    no_data_state  = "Alerting"
    exec_err_state = "Error"

    data {
      ref_id = "A"
      relative_time_range {
        from = var.log_gap_minutes * 60 * 2
        to   = 0
      }
      datasource_uid = data.grafana_data_source.logs.uid
      model = jsonencode({
        refId         = "A"
        datasource    = { type = "loki", uid = data.grafana_data_source.logs.uid }
        expr          = local.log_gap_expr
        queryType     = "instant"
        intervalMs    = 1000
        maxDataPoints = 43200
      })
    }

    data {
      ref_id = "B"
      relative_time_range {
        from = 0
        to   = 0
      }
      datasource_uid = "-100"
      model = jsonencode({
        refId      = "B"
        type       = "threshold"
        datasource = { type = "__expr__", uid = "-100" }
        expression = "A"
        # 0 件のときは NoData 側で拾うので、ここは「1 件未満」を異常とする
        conditions = [{
          type      = "query"
          evaluator = { type = "lt", params = [1] }
          operator  = { type = "and" }
          query     = { params = ["A"] }
          reducer   = { type = "last", params = [] }
        }]
      })
    }

    annotations = {
      summary     = "${var.log_gap_environment} のログが ${var.log_gap_minutes} 分間 1 行も届いていません"
      description = "ホストか compose の alloy が動いていない可能性がある。ホストで docker compose ps と docker compose logs alloy を見る"
    }
    labels = { severity = "critical" }

    notification_settings {
      contact_point = grafana_contact_point.discord.name
      group_by      = ["alertname", "grafana_folder"]
    }
  }
}
