use actix_web::{HttpResponse, ResponseError};
use actix_web::http::StatusCode;
use diesel::result::Error as DieselError;
use std::fmt;
use reqwest::header;

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub status_code: u16,
    pub message: String,
}

impl ApiError {
    pub fn new(status_code: u16, message: String) -> Self {
        Self { status_code, message }
    }
}

impl From<DieselError> for ApiError {
    fn from(err: DieselError) -> ApiError {
        match err {
            DieselError::NotFound
                => ApiError::new(404, "Record not found".to_string()),
            DieselError::DatabaseError(_, e)
                => ApiError::new(409, e.message().to_string()),
            e => {
                error!("Unexpected Diesel Error: {:?}", e);
                ApiError::new(500, "Internal Server Error".to_string())
            }
        }
    }
}

impl From<header::InvalidHeaderValue> for ApiError {
    fn from(error: header::InvalidHeaderValue) -> ApiError {
        ApiError::new(500, format!("Header Parse Error: {}", error))
    }
}


impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> ApiError {
        error!("{:?}", error);
        match error.status() {
            Some(reqwest::StatusCode::UNAUTHORIZED) => ApiError::new(401, "Unauthorized".to_string()),
            None => ApiError::new(401, "Unauthorized".to_string()),
            _ => ApiError::new(500, format!("Client Error({:?}): {}", error.status(), error))
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message.as_str())
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status_code = match StatusCode::from_u16(self.status_code) {
            Ok(status_code) => status_code,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match status_code.as_u16() < 500 {
            true => self.message.clone(),
            false => {
                error!("{:?}", self.message);
                "Internal server Error".to_string()
            },
        };

        HttpResponse::build(status_code)
            .json(json!({ "message": message }))
    }
}
