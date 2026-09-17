# 添付専用。バックアップの保存期限・ロック・認証情報は流用しない。
resource "cloudflare_r2_bucket" "attachments" {
  for_each      = var.attachment_buckets
  account_id    = var.account_id
  name          = each.key
  location      = "apac"
  storage_class = "Standard"
}

resource "cloudflare_r2_bucket_cors" "attachments" {
  for_each    = var.attachment_buckets
  account_id  = var.account_id
  bucket_name = cloudflare_r2_bucket.attachments[each.key].name
  rules = [{
    id = "web-attachments"
    allowed = {
      origins = each.value
      methods = ["GET", "HEAD", "PUT"]
      headers = ["Content-Type", "Content-Length", "If-None-Match"]
    }
    expose_headers  = ["ETag", "Content-Disposition"]
    max_age_seconds = 300
  }]
}

resource "cloudflare_r2_bucket_lifecycle" "attachments" {
  for_each    = var.attachment_buckets
  account_id  = var.account_id
  bucket_name = cloudflare_r2_bucket.attachments[each.key].name
  rules = [{
    id         = "expire-temporary-uploads"
    enabled    = true
    conditions = { prefix = "temporary/" }
    delete_objects_transition = {
      condition = { type = "Age", max_age = 2 * 24 * 60 * 60 }
    }
  }]
}
