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
use actix_web::web::{Data, Json, Path};
use redis::aio::Connection;
use serde::Serialize;
use std::collections::BTreeMap;

type EraCache = BTreeMap<String, String>;

#[derive(Debug, Serialize, PartialEq)]
pub struct EraResponse {
    pub era_index: u32,
    pub total_reward: u128,
    pub total_stake: u128,
    pub total_reward_points: u32,
    pub min_reward_points: u32,
    pub max_reward_points: u32,
    pub avg_reward_points: u32,
}

impl From<EraCache> for EraResponse {
    fn from(data: EraCache) -> Self {
        let zero = "0".to_string();
        EraResponse {
            era_index: data
                .get("era_index")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            total_reward: data
                .get("total_reward")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            total_stake: data
                .get("total_stake")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            total_reward_points: data
                .get("total_reward_points")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            min_reward_points: data
                .get("min_reward_points")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            max_reward_points: data
                .get("max_reward_points")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            avg_reward_points: data
                .get("avg_reward_points")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
        }
    }
}

/// Get a era
pub async fn get_era(
    era_index: Path<u32>,
    cache: Data<RedisPool>,
) -> Result<Json<EraResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let key = format!("{}:era", era_index);
    let mut data: EraCache = redis::cmd("HGETALL")
        .arg(key)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let not_found = format!("era index {} not available", era_index);
    data.insert("era_index".to_string(), era_index.to_string());
    if data.len() == 0 {
        return Err(ApiError::NotFound(not_found));
    }
    respond_json(data.into())
}
