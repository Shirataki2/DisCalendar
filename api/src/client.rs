use crate::error::ApiError;
use reqwest::header;

const API_ROOT_URL: &str = "https://discord.com/api";

pub fn get_client(token: String) -> Result<reqwest::Client, ApiError> {
    let mut headers = header::HeaderMap::new();
    let token = format!("Bearer {}", &token);
    headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(&token)?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(client)
}

pub fn get_bot() -> Result<reqwest::Client, ApiError> {
    let mut headers = header::HeaderMap::new();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN is required");
    let token = format!("Bot {}", &token);
    headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(&token)?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(client)
}

pub fn get_api_url(endpoint: &str) -> String {
    format!("{}{}", API_ROOT_URL, endpoint)
}
