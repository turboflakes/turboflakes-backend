// The MIT License (MIT)
// Copyright © 2021 Aukbit Ltd.
// 
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// 
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
// 
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use actix_web::{error::ResponseError, HttpResponse};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Display, PartialEq)]
#[allow(dead_code)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    InternalServerError(String),
}

/// Automatically convert ApiErrors to external Response Errors
impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::BadRequest(error) => {
                HttpResponse::BadRequest().json::<ErrorResponse>(error.into())
            }
            ApiError::NotFound(message) => {
                HttpResponse::NotFound().json::<ErrorResponse>(message.into())
            }
            ApiError::InternalServerError(error) => {
                HttpResponse::InternalServerError().json::<ErrorResponse>(error.into())
            }
        }
    }
}

/// User-friendly error messages
#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<String>,
}

/// Utility to make transforming a string reference into an ErrorResponse
impl From<&String> for ErrorResponse {
    fn from(error: &String) -> Self {
        ErrorResponse {
            errors: vec![error.into()],
        }
    }
}

/// Cache specific error messages
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("could not get redis connection from pool : {0}")]
    RedisPoolError(mobc::Error<mobc_redis::redis::RedisError>),
    #[error("error parsing string from redis result: {0}")]
    RedisTypeError(mobc_redis::redis::RedisError),
    #[error("error executing redis command: {0}")]
    RedisCMDError(mobc_redis::redis::RedisError),
    #[error("error creating redis client: {0}")]
    RedisClientError(mobc_redis::redis::RedisError),
    #[error("pong response error")]
    RedisPongError,
    #[error("Other error: {0}")]
    Other(String),
}

/// Convert CacheError to Sttring
impl From<CacheError> for String {
    fn from(error: CacheError) -> Self {
        format!("{}", error).to_string()
    }
}

/// Convert CacheError to ApiErrors
impl From<CacheError> for ApiError {
    fn from(error: CacheError) -> Self {
        ApiError::InternalServerError(error.into())
    }
}

/// Syncronization specific error messages
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("cache error: {0}")]
    CacheError(#[from] CacheError),
    #[error("substrate_subxt error: {0}")]
    SubxtError(#[from] substrate_subxt::Error),
    #[error("Other error: {0}")]
    Other(String),
}