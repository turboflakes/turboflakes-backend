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

use crate::cache::{get_conn, RedisPool};
use crate::errors::{ApiError, CacheError};
use crate::helpers::respond_json;
use crate::sync::sync;
use actix_web::web::{Data, Json};
use redis::aio::Connection;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct InfoResponse {
    pub pkg_name: String,
    pub pkg_version: String,
    pub api_path: String,
    pub chain: ChainDetailsResponse,
    pub cache: CacheInfoResponse,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ChainDetailsResponse {
    pub name: String,
    pub token_symbol: String,
    pub token_decimals: u8,
    pub ss58_format: u8,
    pub substrate_node_url: String,
}

impl From<BTreeMap<String, String>> for ChainDetailsResponse {
    fn from(data: BTreeMap<String, String>) -> Self {
        let zero = "0".to_string();
        ChainDetailsResponse {
            name: data.get("name").unwrap_or(&"".to_string()).to_string(),
            token_symbol: data
                .get("token_symbol")
                .unwrap_or(&"".to_string())
                .to_string(),
            token_decimals: data
                .get("token_decimals")
                .unwrap_or(&zero)
                .parse::<u8>()
                .unwrap_or_default(),
            ss58_format: data
                .get("ss58_format")
                .unwrap_or(&zero)
                .parse::<u8>()
                .unwrap_or_default(),
            substrate_node_url: data
                .get("substrate_node_url")
                .unwrap_or(&"".to_string())
                .to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CacheInfoResponse {
    pub syncing: bool,
    pub syncing_started_at: u32,
    pub syncing_finished_at: u32,
    pub validators: u32,
    pub nominators: u32,
}

impl From<BTreeMap<String, String>> for CacheInfoResponse {
    fn from(data: BTreeMap<String, String>) -> Self {
        let zero = "0".to_string();
        CacheInfoResponse {
            syncing: data
                .get("syncing")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
            syncing_started_at: data
                .get("syncing_started_at")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            syncing_finished_at: data
                .get("syncing_finished_at")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            validators: data
                .get("validators")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            nominators: data
                .get("nominators")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
        }
    }
}

/// Handler to get information about the service
pub async fn get_info(cache: Data<RedisPool>) -> Result<Json<InfoResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let cache_info: BTreeMap<String, String> = redis::cmd("HGETALL")
        .arg(sync::Key::Info)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let chain_info: BTreeMap<String, String> = redis::cmd("HGETALL")
        .arg(sync::Key::Network)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    respond_json(InfoResponse {
        pkg_name: env!("CARGO_PKG_NAME").into(),
        pkg_version: env!("CARGO_PKG_VERSION").into(),
        api_path: "/api/v1".into(),
        chain: chain_info.into(),
        cache: cache_info.into(),
    })
}
