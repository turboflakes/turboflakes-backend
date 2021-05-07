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
use crate::sync::{stats, sync};
use actix_web::web::{Data, Json, Path, Query};
use redis::aio::Connection;
use serde::{de::Deserializer, Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};
use substrate_subxt::{sp_runtime::AccountId32, staking::EraIndex};

type ValidatorCache = BTreeMap<String, String>;
type ValidatorEraCache = BTreeMap<String, String>;

#[derive(Debug, Serialize, PartialEq)]
pub struct Validator {
    pub stash: String,
    pub controller: String,
    pub name: String,
    pub own_stake: u128,
    pub inclusion_rate: f32,
    pub avg_reward_points: f64,
    pub commission: u32,
    pub blocked: bool,
    pub active: bool,
    pub reward_staked: bool,
}

impl From<ValidatorCache> for Validator {
    fn from(data: ValidatorCache) -> Self {
        let zero = "0".to_string();
        Validator {
            stash: data.get("stash").unwrap_or(&"".to_string()).to_string(),
            controller: data
                .get("controller")
                .unwrap_or(&"".to_string())
                .to_string(),
            name: data.get("name").unwrap_or(&"".to_string()).to_string(),
            own_stake: data
                .get("own_stake")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            inclusion_rate: data
                .get("inclusion_rate")
                .unwrap_or(&zero)
                .parse::<f32>()
                .unwrap_or_default(),
            avg_reward_points: data
                .get("avg_reward_points")
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

type ValidatorResponse = Validator;

/// Get a validator
pub async fn get_validator(
    stash: Path<String>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let stash = AccountId32::from_str(&*stash.to_string())?;
    let mut data: ValidatorCache = redis::cmd("HGETALL")
        .arg(sync::Key::Validator(stash.clone()))
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
    pub stakers: u32,
    pub others_stake_clipped: u128,
    pub stakers_clipped: u32,
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
            stakers: data
                .get("stakers")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            others_stake_clipped: data
                .get("others_stake_clipped")
                .unwrap_or(&zero)
                .parse::<u128>()
                .unwrap_or_default(),
            stakers_clipped: data
                .get("stakers_clipped")
                .unwrap_or(&zero)
                .parse::<u32>()
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
                .arg(sync::Key::ValidatorAtEraScan(stash.clone()))
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

#[derive(Debug, Deserialize, Clone, PartialEq)]
enum Queries {
    All = 1,
    Active = 2,
    Board = 3,
}

impl std::fmt::Display for Queries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Active => write!(f, "active"),
            Self::Board => write!(f, "board"),
        }
    }
}

/// Weight can be any value in a 10-point scale. Higher the weight more important
/// is the criteria to the user
type Weight = u32;

/// Weights represent an array of points, where the points in each position represents
/// the weight for the respective criteria
/// Position 0 - Higher Inclusion rate is preferrable
/// Position 1 - Lower Commission is preferrable
/// Position 2 - Higher Reward Points is preferrable
/// Position 3 - If reward is staked is preferrable
/// Position 4 - If in active set is preferrable
/// Position 5 - Higher own stake is preferrable
type Weights = Vec<Weight>;

/// Current weighs capacity
const WEIGHTS_CAPACITY: usize = 6;

// Number of elements to return
type Quantity = u32;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Params {
    q: Queries,
    #[serde(default)]
    #[serde(deserialize_with = "parse_weights")]
    w: Weights,
    n: Quantity,
}

fn parse_weights<'de, D>(d: D) -> Result<Weights, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(|x: Option<_>| {
        let weights_as_csv = x.unwrap_or("".to_string());

        let mut weights_as_strvec: Vec<&str> = weights_as_csv.split(",").collect();
        weights_as_strvec.resize(WEIGHTS_CAPACITY, "5");

        let mut weights: Weights = Vec::with_capacity(WEIGHTS_CAPACITY);
        for i in 0..WEIGHTS_CAPACITY {
            let weight: u32 = weights_as_strvec[i].to_string().parse().unwrap_or(5);
            let weight = if weight > 10 { 10 } else { weight };
            weights.push(weight);
        }
        weights
    })
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorsResponse {
    pub data: Vec<String>,
}

fn get_board_name(weights: &Weights) -> String {
    weights
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if i == 0 {
                return x.to_string();
            }
            format!(",{}", x)
        })
        .collect()
}

/// Normalize inclusion rate between 0 - 100
fn normaliza_inclusion(inclusion_rate: f32) -> f64 {
    (inclusion_rate * 100.0) as f64
}

/// Normalize commission between 0 - 100
/// lower commission the better
fn normaliza_commission(commission: u32) -> f64 {
    100.0 - (commission / 10000000) as f64
}

/// Normalize boolean flag between 0 - 100
fn normalize_flag(flag: bool) -> f64 {
    (flag as u32 * 100) as f64
}

/// Normalize average reward points between 0 - 1000
fn normalize_avg_reward_points(
    avg_reward_points: f64,
    min_points_limit: f64,
    max_points_limit: f64,
) -> f64 {
    if avg_reward_points == 0.0 {
        return 0.0;
    }
    100.0 * ((avg_reward_points - min_points_limit) / (max_points_limit - min_points_limit))
}

/// Normalize own stake points between 0 - 1000
fn normalize_own_stake(own_stake: u128, min_own_stake: u128, max_own_stake: u128) -> f64 {
    if own_stake == 0 {
        return 0.0;
    }
    let value =
        (own_stake as f64 - min_own_stake as f64) / (max_own_stake as f64 - min_own_stake as f64);
    value * 100.0
}

async fn calculate_avg_points(cache: Data<RedisPool>, name: &str) -> Result<f64, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(EraIndex, u32)> = redis::cmd("ZRANGE")
        .arg(sync::Key::BoardAtEra(0, name.to_string()))
        .arg("-inf")
        .arg("+inf")
        .arg("BYSCORE")
        .arg("WITHSCORES")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    // Convert Vec<(EraIndex, u32)> to Vec<u32> to easily calculate average
    let scores: Vec<u32> = v.into_iter().map(|(_, score)| score).collect();
    let avg = stats::mean(&scores);
    Ok(avg)
}

async fn calculate_min_own_stake_limit(
    cache: Data<RedisPool>,
    name: &str,
) -> Result<u128, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(String, u128)> = redis::cmd("ZRANGE")
        .arg(sync::Key::BoardAtEra(0, name.to_string()))
        .arg("-inf")
        .arg("+inf")
        .arg("BYSCORE")
        .arg("LIMIT")
        .arg("0")
        .arg("1")
        .arg("WITHSCORES")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    if v.len() == 0 {
        return Ok(0);
    }
    Ok(v[0].1)
}

async fn calculate_max_own_stake_limit(
    cache: Data<RedisPool>,
    name: &str,
) -> Result<u128, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(String, u128)> = redis::cmd("ZRANGE")
        .arg(sync::Key::BoardAtEra(0, name.to_string()))
        .arg("+inf")
        .arg("-inf")
        .arg("BYSCORE")
        .arg("REV")
        .arg("LIMIT")
        .arg("0")
        .arg("1")
        .arg("WITHSCORES")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    if v.len() == 0 {
        return Ok(0);
    }
    Ok(v[0].1)
}

async fn generate_board(
    era_index: EraIndex,
    weights: &Weights,
    cache: Data<RedisPool>,
) -> Result<(), ApiError> {
    let mut conn = get_conn(&cache).await?;

    let max_points_limit = calculate_avg_points(cache.clone(), sync::BOARD_MAX_POINTS_ERAS).await?;
    let min_points_limit = calculate_avg_points(cache.clone(), sync::BOARD_MIN_POINTS_ERAS).await?;
    let min_own_stake_limit =
        calculate_min_own_stake_limit(cache.clone(), sync::BOARD_OWN_STAKE_VALIDATORS).await?;
    let max_own_stake_limit =
        calculate_max_own_stake_limit(cache.clone(), sync::BOARD_OWN_STAKE_VALIDATORS).await?;

    let stashes: Vec<String> = redis::cmd("ZRANGE")
        .arg(sync::Key::BoardAtEra(
            era_index,
            sync::BOARD_ALL_VALIDATORS.to_string(),
        ))
        .arg("-inf")
        .arg("+inf")
        .arg("BYSCORE")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let key = sync::Key::BoardAtEra(era_index, get_board_name(weights));
    for stash in stashes {
        let stash = AccountId32::from_str(&*stash.to_string())?;
        let data: ValidatorCache = redis::cmd("HGETALL")
            .arg(sync::Key::Validator(stash.clone()))
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;

        let validator: Validator = data.into();
        // If the validator does not accept nominations
        // score is not given
        if validator.blocked {
            continue;
        }

        let score = normaliza_inclusion(validator.inclusion_rate) * weights[0] as f64
            + normaliza_commission(validator.commission) * weights[1] as f64
            + normalize_avg_reward_points(
                validator.avg_reward_points,
                min_points_limit,
                max_points_limit,
            ) * weights[2] as f64
            + normalize_flag(validator.reward_staked) * weights[3] as f64
            + normalize_flag(validator.active) * weights[4] as f64
            + normalize_own_stake(
                validator.own_stake,
                min_own_stake_limit,
                max_own_stake_limit,
            ) * weights[5] as f64;

        let _: () = redis::cmd("ZADD")
            .arg(key.to_string())
            .arg(score) // score
            .arg(stash.to_string()) // member
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
    }

    Ok(())
}

/// Get a validators
pub async fn get_validators(
    params: Query<Params>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorsResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let era_index: EraIndex = redis::cmd("GET")
        .arg(sync::Key::ActiveEra)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let key = match params.q {
        Queries::Active => {
            sync::Key::BoardAtEra(era_index, sync::BOARD_ACTIVE_VALIDATORS.to_string())
        }
        Queries::All => sync::Key::BoardAtEra(era_index, sync::BOARD_ALL_VALIDATORS.to_string()),
        Queries::Board => sync::Key::BoardAtEra(era_index, get_board_name(&params.w)),
    };

    let exists: bool = redis::cmd("EXISTS")
        .arg(key.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    if !exists {
        // Generate and cache leaderboard
        generate_board(era_index, &params.w, cache).await?;
    }

    let stashes: Vec<String> = redis::cmd("ZRANGE")
        .arg(key.clone())
        .arg("+inf")
        .arg("0")
        .arg("BYSCORE")
        .arg("REV")
        .arg("LIMIT")
        .arg("0")
        .arg(params.n)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    respond_json(ValidatorsResponse { data: stashes })
}
