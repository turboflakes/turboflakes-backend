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
use crate::helpers::{respond_json};
use crate::sync::sync::Key;
use actix_web::web::{Data, Json, Path, Query};
use redis::aio::Connection;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};
use substrate_subxt::{sp_runtime::AccountId32, staking::EraIndex};

type ValidatorCache = BTreeMap<String, String>;
type ValidatorEraCache = BTreeMap<String, String>;

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorResponse {
    pub stash: String,
    pub controller: String,
    pub name: String,
    pub inclusion_rate: f32,
    pub mean_reward_points: f64,
    pub commission: u32,
    pub blocked: bool,
    pub active: bool,
    pub reward_staked: bool,
}

impl From<ValidatorCache> for ValidatorResponse {
    fn from(data: ValidatorCache) -> Self {
        let zero = "0".to_string();
        ValidatorResponse {
            stash: data.get("stash").unwrap_or(&"".to_string()).to_string(),
            controller: data
                .get("controller")
                .unwrap_or(&"".to_string())
                .to_string(),
            name: data.get("name").unwrap_or(&"".to_string()).to_string(),
            inclusion_rate: data
                .get("inclusion_rate")
                .unwrap_or(&zero)
                .parse::<f32>()
                .unwrap_or_default(),
            mean_reward_points: data
                .get("mean_reward_points")
                .unwrap_or(&zero)
                .parse::<f64>()
                .unwrap_or_default(),
            commission: data
                .get("commission")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            blocked: data
                .get("blocked")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
            active: data
                .get("active")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
            reward_staked: data
                .get("reward_staked")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
        }
    }
}

/// Get a validator
pub async fn get_validator(
    stash: Path<String>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let stash = AccountId32::from_str(&*stash.to_string())?;
    let mut data: ValidatorCache = redis::cmd("HGETALL")
        .arg(Key::Validator(stash.clone()))
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let not_found = format!("validator stash {} not available", stash);
    data.insert("stash".to_string(), stash.to_string());
    if data.len() == 0 {
        return Err(ApiError::NotFound(not_found));
    }
    respond_json(data.into())
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorEra {
    pub era_index: u32,
    pub own_stake: u128,
    pub total_stake: u128,
    pub others_stake: u128,
    pub reward_points: u32,
    pub commission: u32,
    pub blocked: bool,
    pub active: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorEraResponse {
    pub stash: String,
    pub eras: Vec<ValidatorEra>,
}

impl From<ValidatorEraCache> for ValidatorEra {
    fn from(data: ValidatorEraCache) -> Self {
        let zero = "0".to_string();
        ValidatorEra {
            era_index: data
                .get("era_index")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            own_stake: data
                .get("own_stake")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            total_stake: data
                .get("total_stake")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            others_stake: data
                .get("others_stake")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            reward_points: data
                .get("reward_points")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            commission: data
                .get("commission")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            blocked: data
                .get("blocked")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
            active: data
                .get("active")
                .unwrap_or(&zero)
                .parse::<bool>()
                .unwrap_or_default(),
        }
    }
}

/// Get a validator eras
pub async fn get_validator_eras(
    stash: Path<String>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorEraResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;

    let stash = AccountId32::from_str(&*stash.to_string())?;
    let mut eras: Vec<ValidatorEra> = vec![];
    let mut optional = Some(-1);
    while let Some(i) = optional {
        if i == 0 {
            optional = None;
        } else {
            // First time on the loop cursor is always 0
            let cursor = if i == -1 { 0 } else { i };

            // Scan redis
            let (cursor, keys): (i32, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(Key::ValidatorAtEraScan(stash.clone()))
                .arg("COUNT")
                .arg("100")
                .query_async(&mut conn as &mut Connection)
                .await
                .map_err(CacheError::RedisCMDError)?;

            optional = Some(cursor);

            for key in keys {
                let mut data: ValidatorEraCache = redis::cmd("HGETALL")
                    .arg(key.clone())
                    .query_async(&mut conn as &mut Connection)
                    .await
                    .map_err(CacheError::RedisCMDError)?;

                let not_found = format!("cache key {} not available", key);
                if data.len() == 0 {
                    return Err(ApiError::NotFound(not_found));
                }
                if let Some(x) = key.find(':') {
                    data.insert("era_index".to_string(), String::from(&key[..x]));
                }
                eras.push(data.into());
            }
        }
    }

    // Sort eras by era_index
    eras.sort_by(|a, b| b.era_index.cmp(&a.era_index));
    respond_json(ValidatorEraResponse {
        stash: stash.to_string(),
        eras: eras,
    })
}

#[derive(Debug, Deserialize, PartialEq)]
enum Queries {
    All = 1,
    Active = 2,
}

#[derive(Debug, Deserialize)]
pub struct Params {
    q: Queries,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorsResponse {
    pub data: Vec<String>,
}

/// Get a validators
pub async fn get_validators(
    params: Query<Params>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorsResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let era_index: EraIndex = redis::cmd("GET")
        .arg(Key::ActiveEra)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let key = match params.q {
        Queries::Active => Key::AllValidatorsByEra(era_index),
        Queries::All => Key::ActiveValidatorsByEra(era_index),
    };
    let stashes: Vec<String> = redis::cmd("SMEMBERS")
        .arg(key)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    respond_json(ValidatorsResponse {
        data: stashes,
    })
}
