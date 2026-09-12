//! 外部 URL 向けの SSRF 対策。DNS 検査済みのアドレスに接続を固定する。
use reqwest::{Client, Url};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub fn parse_url(raw: &str) -> Result<Url, &'static str> {
    let url = Url::parse(raw).map_err(|_| "URL が正しくありません")?;
    if raw.len() > 2048
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("認証情報・フラグメントを含まない HTTP(S) URL を指定してください");
    }
    Ok(url)
}

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (18..=19).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 192 && b == 88 && c == 99))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // IANA の global unicast のみ。変換・トンネル・文書用の範囲も拒否する。
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub async fn client_for(url: &Url) -> Result<Client, &'static str> {
    let host = url.host_str().ok_or("ホストがありません")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let port = url.port_or_known_default().ok_or("ポートがありません")?;
    #[cfg(feature = "webhook-e2e")]
    if host == "webhook.test" {
        return Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .resolve(host, SocketAddr::from(([127, 0, 0, 1], port)))
            .build()
            .map_err(|_| "テストクライアントを作成できません");
    }
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "DNS の解決に失敗しました")?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !is_public(a.ip())) {
        return Err("公開インターネットのアドレスだけを指定できます");
    }
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| "HTTP クライアントを作成できません")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_internal_and_special_addresses_and_unsafe_urls() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "198.18.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "64:ff9b::a00:1",
            "2002:7f00:1::1",
            "2001:db8::1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(is_public(ip.parse().unwrap()));
        }
        for url in [
            "file:///etc/passwd",
            "ftp://example.com",
            "https://user:pass@example.com",
            "https://example.com/#secret",
        ] {
            assert!(parse_url(url).is_err());
        }
        // URL パーサーが整数形式の IPv4 も正規化する。
        assert_eq!(
            parse_url("http://2130706433").unwrap().host_str(),
            Some("127.0.0.1")
        );
    }
}
