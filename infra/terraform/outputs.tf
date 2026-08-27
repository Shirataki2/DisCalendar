output "backup_bucket_name" {
  description = "バックアップ用 R2 バケットの名前 (ホストの backup.env の R2_BUCKET に入れる)"
  value       = cloudflare_r2_bucket.backups.name
}

output "r2_s3_endpoint" {
  description = "R2 の S3 互換エンドポイント (aws-cli の --endpoint-url、backend.hcl の endpoints)"
  value       = "https://${var.account_id}.r2.cloudflarestorage.com"
}

output "backup_retention_days" {
  description = "バックアップが自動削除されるまでの日数"
  value       = var.backup_retention_days
}

output "backup_lock_days" {
  description = "バックアップを削除・上書きできない日数 (バケットロック)"
  value       = var.backup_lock_days
}
