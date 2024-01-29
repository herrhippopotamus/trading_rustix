use actix_web::{
    error::ResponseError,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use anyhow;
use std::fmt;

#[derive(Debug)]
pub struct RustixErr {
    pub err: anyhow::Error,
    pub status: u16,
}
impl fmt::Display for RustixErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Use `self.number` to refer to each positional data point.
        write!(f, "status {} - err {:?})", self.status, self.err)
    }
}
impl std::error::Error for RustixErr {}
impl ResponseError for RustixErr {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }
}
impl From<anyhow::Error> for RustixErr {
    fn from(err: anyhow::Error) -> Self {
        Self { err, status: 500 }
    }
}
impl RustixErr {
    pub fn new(err: anyhow::Error, status: u16) -> Self {
        Self { err, status }
    }
}
