# Grafana Cloud のアラート (ログベース) を Terraform で管理する (#104)。
# Cloudflare (infra/terraform/) とは provider も認証情報も別なので、state を分けて別のルートモジュールにしてある。
# provider はリソースの形が版によって動くのでバージョンを固定し、上げるときに plan で確認する。

terraform {
  required_version = ">= 1.11, < 2.0"

  required_providers {
    grafana = {
      source  = "grafana/grafana"
      version = "4.45.2"
    }
  }

  # state は Cloudflare 側と同じ R2 のバケットに、別のキーで置く。
  # バケット名とエンドポイントは backend.hcl で渡す (infra/README.md)。
  #   terraform init -backend-config=backend.hcl
  backend "s3" {
    key = "discalendar/grafana.tfstate"
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

provider "grafana" {
  # スタックの Grafana の URL (https://<stack>.grafana.net) と、そこで作るサービスアカウントトークン。
  # トークンは環境変数 GRAFANA_AUTH で渡す (state にも平文で載るので tfvars にもコードにも書かない)
  url = var.grafana_url
}
