# Cloudflare の設定を Terraform で管理する下地 (#102)。
# provider は 5 系で書き直されていてリソースごとに粗さが残るため、バージョンは固定して上げるときに plan で確認する。

terraform {
  required_version = ">= 1.11, < 2.0"

  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "5.24.0"
    }
  }

  # state は R2 の S3 互換 API に置く。バケット名とエンドポイントは環境ごとに違うので
  # partial configuration にしてあり、init のときに backend.hcl で渡す (infra/README.md)。
  #   terraform init -backend-config=backend.hcl
  # アクセスキーは環境変数 AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY で渡す (ファイルに書かない)。
  backend "s3" {
    key = "discalendar/terraform.tfstate"
    # R2 は AWS のリージョンも IMDS も持たないので、AWS 前提の検証はすべて飛ばす
    region                      = "auto"
    use_path_style              = true
    skip_credentials_validation = true
    skip_metadata_api_check     = true
    skip_region_validation      = true
    skip_requesting_account_id  = true
    # R2 は AWS SDK が既定で付ける CRC32 チェックサムを受け付けない
    skip_s3_checksum = true
    # state のロックはバケット内のロックファイルで行う (DynamoDB の代わり。R2 の条件付き書き込みを使う)
    use_lockfile = true
  }
}

provider "cloudflare" {
  # API トークンは環境変数 CLOUDFLARE_API_TOKEN で渡す。
  # ここに書くと state (R2 の上) にも平文で載るので、コードにも tfvars にも入れない
}
