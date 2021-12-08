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
use crate::sync::{stats, sync, sync::EraIndex};
use actix_web::web::{Data, Json, Path, Query};
use log::{error, warn};
use redis::aio::Connection;
use serde::{de::Deserializer, Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};
use subxt::sp_runtime::AccountId32;

type ValidatorCache = BTreeMap<String, String>;
type ValidatorEraCache = BTreeMap<String, String>;

#[derive(Debug, Serialize, PartialEq)]
pub struct Validator {
    pub stash: String,
    pub controller: String,
    pub name: String,
    pub own_stake: u128,
    pub nominators: u32,
    pub nominators_stake: u128,
    pub inclusion_rate: f32,
    pub avg_reward_points: f64,
    pub commission: u32,
    pub blocked: bool,
    pub active: bool,
    pub reward_staked: bool,
    pub judgements: u32,
    pub sub_accounts: u32,
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
            nominators: data
                .get("nominators")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            nominators_stake: data
                .get("nominators_stake")
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
            judgements: data
                .get("judgements")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
            sub_accounts: data
                .get("sub_accounts")
                .unwrap_or(&zero)
                .parse::<u32>()
                .unwrap_or_default(),
        }
    }
}

type ValidatorResponse = Validator;

/// Get a validator
pub async fn get_validator(
    stash: Path<String>,
    _params: Query<Params>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let stash = AccountId32::from_str(&*stash.to_string())?;
    let mut data: ValidatorCache = redis::cmd("HGETALL")
        .arg(sync::Key::Validator(stash.clone()))
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    if data.len() == 0 {
        let msg = format!("Validator account with address {} not found", stash);
        warn!("{}", msg);
        return Err(ApiError::NotFound(msg));
    }
    data.insert("stash".to_string(), stash.to_string());

    respond_json(data.into())
}

type BoardLimitsCache = BTreeMap<String, f64>;

#[derive(Debug, Serialize, PartialEq, Copy, Clone)]
pub struct Interval {
    pub min: f64,
    pub max: f64,
}

impl Default for Interval {
    fn default() -> Interval {
        Interval {
            min: 0.0_f64,
            max: 0.0_f64,
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.min, self.max)
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct BoardLimits {
    pub inclusion_rate: Interval,
    pub commission: Interval,
    pub nominators: Interval,
    pub avg_reward_points: Interval,
    pub reward_staked: Interval,
    pub active: Interval,
    pub own_stake: Interval,
    pub total_stake: Interval,
    pub judgements: Interval,
    pub sub_accounts: Interval,
}

impl Default for BoardLimits {
    fn default() -> BoardLimits {
        BoardLimits {
            inclusion_rate: Interval::default(),
            commission: Interval::default(),
            nominators: Interval::default(),
            avg_reward_points: Interval::default(),
            reward_staked: Interval::default(),
            active: Interval::default(),
            own_stake: Interval::default(),
            total_stake: Interval::default(),
            judgements: Interval::default(),
            sub_accounts: Interval::default(),
        }
    }
}

impl std::fmt::Display for BoardLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Note: the position of the traits is important, it should be the same as the position in weights
        write!(
            f,
            "{},{},{},{},{},{},{},{},{},{}",
            self.inclusion_rate.to_string(),
            self.commission.to_string(),
            self.nominators.to_string(),
            self.avg_reward_points.to_string(),
            self.reward_staked.to_string(),
            self.active.to_string(),
            self.own_stake.to_string(),
            self.total_stake.to_string(),
            self.judgements.to_string(),
            self.sub_accounts.to_string()
        )
    }
}

impl From<&Intervals> for BoardLimits {
    fn from(data: &Intervals) -> Self {
        BoardLimits {
            inclusion_rate: *data.get(0).unwrap_or(&Interval::default()),
            commission: *data.get(1).unwrap_or(&Interval::default()),
            nominators: *data.get(2).unwrap_or(&Interval::default()),
            avg_reward_points: *data.get(3).unwrap_or(&Interval::default()),
            reward_staked: *data.get(4).unwrap_or(&Interval::default()),
            active: *data.get(5).unwrap_or(&Interval::default()),
            own_stake: *data.get(6).unwrap_or(&Interval::default()),
            total_stake: *data.get(7).unwrap_or(&Interval::default()),
            judgements: *data.get(8).unwrap_or(&Interval::default()),
            sub_accounts: *data.get(9).unwrap_or(&Interval::default()),
        }
    }
}

impl From<BoardLimitsCache> for BoardLimits {
    fn from(data: BoardLimitsCache) -> Self {
        let default_min = 0.0_f64;
        let default_max = 100.0_f64;
        BoardLimits {
            inclusion_rate: Interval {
                min: 0.0_f64,
                max: 1.0_f64,
            },
            commission: Interval {
                min: 0.0_f64,
                max: COMMISSION_PLANCK as f64,
            },
            nominators: Interval {
                min: 0.0_f64,
                max: NOMINATORS_OVERSUBSCRIBED_THRESHOLD as f64,
            },
            avg_reward_points: Interval {
                min: *data.get("min_avg_reward_points").unwrap_or(&default_min),
                max: *data.get("max_avg_reward_points").unwrap_or(&default_max),
            },
            reward_staked: Interval {
                min: 0.0_f64,
                max: 1.0_f64,
            },
            active: Interval {
                min: 0.0_f64,
                max: 1.0_f64,
            },
            own_stake: Interval {
                min: *data.get("min_own_stake").unwrap_or(&default_min),
                max: *data.get("max_own_stake").unwrap_or(&default_max),
            },
            total_stake: Interval {
                min: *data.get("min_total_stake").unwrap_or(&default_min),
                max: *data.get("max_total_stake").unwrap_or(&default_max),
            },
            judgements: Interval {
                min: *data.get("min_judgements").unwrap_or(&default_min),
                max: *data.get("max_judgements").unwrap_or(&default_max),
            },
            sub_accounts: Interval {
                min: *data.get("min_sub_accounts").unwrap_or(&default_min),
                max: *data.get("max_sub_accounts").unwrap_or(&default_max),
            },
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub enum Status {
    Ok = 1,
    NotReady = 2,
    NotFound = 3,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorRankResponse {
    pub stash: String,
    pub rank: i64,
    pub scores: Vec<f64>,
    pub status: Status,
    pub status_msg: String,
}

/// Get a validator rank
pub async fn get_validator_rank(
    stash: Path<String>,
    params: Query<Params>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorRankResponse>, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let stash = AccountId32::from_str(&*stash.to_string())?;
    // Set field rank if params are correctly defined
    let board_name = match params.q {
        Queries::Board => get_board_name(&params.w, Some(&params.i)),
        _ => {
            let msg = format!("Parameter q must be equal to one of the options: [Board]");
            warn!("{}", msg);
            return Err(ApiError::BadRequest(msg));
        }
    };

    let era_index: EraIndex = redis::cmd("GET")
        .arg(sync::Key::ActiveEra)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    let key = sync::Key::BoardAtEra(era_index, board_name.clone());

    // Sometimes the board is still not available since it has been
    // requested at the same time and is still being generated. For these situations
    // just respond with a not_ready status
    if let redis::Value::Int(0) = redis::cmd("EXISTS")
        .arg(key.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?
    {
        let msg = format!(
            "The rank for stash {} is not yet available. Wait a second and try again.",
            stash
        );
        warn!("{}", msg);
        return respond_json(ValidatorRankResponse {
            stash: stash.to_string(),
            rank: 0,
            scores: Vec::new(),
            status: Status::NotReady,
            status_msg: msg,
        });
    }

    // Get rank
    let rank = match redis::cmd("ZREVRANK")
        .arg(key.clone())
        .arg(stash.to_string())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?
    {
        redis::Value::Int(mut rank) => {
            // Redis rank is index based
            rank += 1;
            rank
        }
        _ => {
            let msg = format!("The rank for stash {} is not found.", stash);
            warn!("{}", msg);
            return respond_json(ValidatorRankResponse {
                stash: stash.to_string(),
                rank: 0,
                scores: Vec::new(),
                status: Status::NotFound,
                status_msg: msg,
            });

            // return Err(ApiError::NotFound(msg));
        }
    };

    // Check if scores key is already available
    let key_scores = sync::Key::BoardAtEra(era_index, format!("{}:scores", board_name));
    if let redis::Value::Int(0) = redis::cmd("HEXISTS")
        .arg(key_scores.to_string())
        .arg(stash.to_string())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?
    {
        let msg = format!(
            "The scores for stash {} are not yet available. Wait a second and try again.",
            stash
        );
        warn!("{}", msg);
        return respond_json(ValidatorRankResponse {
            stash: stash.to_string(),
            rank: 0,
            scores: Vec::new(),
            status: Status::NotReady,
            status_msg: msg,
        });
    }

    // Get scores
    let scores_str = match redis::cmd("HGET")
        .arg(key_scores.to_string())
        .arg(stash.to_string())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?
    {
        redis::Value::Data(scores) => String::from_utf8(scores).unwrap(),
        _ => {
            let msg = format!("Validator scores with address {} not found", stash);
            warn!("{}", msg);
            return Err(ApiError::NotFound(msg));
        }
    };

    let scores_vec: Vec<&str> = scores_str.split(",").collect();
    let scores: Vec<f64> = scores_vec
        .iter()
        .map(|x| x.parse::<f64>().unwrap_or_default())
        .collect();

    respond_json(ValidatorRankResponse {
        stash: stash.to_string(),
        rank: rank,
        scores: scores,
        status: Status::Ok,
        status_msg: "".to_string(),
    })
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

                if data.len() == 0 {
                    let msg = format!("cache key {} not available", key);
                    error!("{}", msg);
                    continue;
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
    Other = 4,
}

impl std::fmt::Display for Queries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Active => write!(f, "active"),
            Self::Board => write!(f, "board"),
            Self::Other => write!(f, "other"),
        }
    }
}

// TODO: get this constants from chain
const NOMINATORS_OVERSUBSCRIBED_THRESHOLD: u32 = 256;
const COMMISSION_PLANCK: u32 = 1000000000;

/// Weight can be any value in a 10-point scale. Higher the weight more important
/// is the criteria to the user
type Weight = u32;

/// Weights represent an array of points, where the points in each position represents
/// the weight for the respective criteria
/// Position 0 - Higher Inclusion rate is preferrable
/// Position 1 - Lower Commission is preferrable
/// Position 2 - Lower Nominators is preferrable (limit to 256 -> oversubscribed)
/// Position 3 - Higher Reward Points is preferrable
/// Position 4 - If reward is staked is preferrable
/// Position 5 - If in active set is preferrable
/// Position 6 - Higher own stake is preferrable
/// Position 7 - Lower total stake is preferrable
/// Position 8 - Higher number of Reasonable or KnownGood judgements is preferrable
/// Position 9 - Lower number of sub-accounts is preferrable
type Weights = Vec<Weight>;

type Intervals = Vec<Interval>;

/// Current weighs capacity
const WEIGHTS_CAPACITY: usize = 10;

/// Current limits capacity
const INTERVALS_CAPACITY: usize = 10;

// Number of elements to return
type Quantity = u32;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Params {
    #[serde(default = "default_queries")]
    q: Queries,
    #[serde(default = "default_weights")]
    #[serde(deserialize_with = "parse_weights")]
    w: Weights,
    #[serde(default = "default_intervals")]
    #[serde(deserialize_with = "parse_intervals")]
    i: Intervals,
    #[serde(default)]
    n: Quantity,
}

fn default_queries() -> Queries {
    Queries::Other
}

fn default_weights() -> Weights {
    vec![0; WEIGHTS_CAPACITY]
}

fn default_intervals() -> Intervals {
    vec![]
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
            let weight: u32 = weights_as_strvec.get(i).unwrap_or(&"0").parse().unwrap();
            let weight = if weight > 9 { 9 } else { weight };
            weights.push(weight);
        }
        weights
    })
}

fn parse_intervals<'de, D>(d: D) -> Result<Intervals, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(|x: Option<_>| {
        let intervals_as_csv = x.unwrap_or("".to_string());
        let mut intervals_as_strvec: Vec<&str> = intervals_as_csv.split(",").collect();
        intervals_as_strvec.resize(INTERVALS_CAPACITY, "0");
        let mut intervals: Intervals = Vec::with_capacity(INTERVALS_CAPACITY);
        for i in 0..INTERVALS_CAPACITY {
            let interval_as_strvec: Vec<&str> = intervals_as_strvec[i].split(":").collect();
            let interval = Interval {
                min: interval_as_strvec.get(0).unwrap_or(&"0").parse().unwrap(),
                max: interval_as_strvec.get(1).unwrap_or(&"0").parse().unwrap(),
            };
            intervals.push(interval);
        }
        intervals
    })
}

#[derive(Debug, Serialize, PartialEq)]
pub struct MetaResponse {
    pub limits: String,
}

impl Default for MetaResponse {
    fn default() -> MetaResponse {
        MetaResponse {
            limits: String::default(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValidatorsResponse {
    pub addresses: Vec<String>,
    pub meta: MetaResponse,
}

fn get_board_name(weights: &Weights, intervals: Option<&Intervals>) -> String {
    match intervals {
        Some(i) => {
            if i.is_empty() {
                return format!("{}", weights_to_string(weights));
            }
            format!("{}|{}", weights_to_string(weights), intervals_to_string(i),)
        }
        None => format!("{}", weights_to_string(weights)),
    }
}

fn weights_to_string(weights: &Weights) -> String {
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

fn intervals_to_string(intervals: &Intervals) -> String {
    intervals
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if i == 0 {
                return format!("{}", x);
            }
            format!(",{}", x)
        })
        .collect()
}

/// Normalize value between min and max
fn normalize_value(value: f64, min: f64, max: f64) -> f64 {
    if value == 0.0 || value < min {
        return 0.0;
    }
    if value > max {
        return 1.0;
    }
    (value - min) / (max - min)
}

/// Reverse normalization
fn reverse_normalize_value(value: f64, min: f64, max: f64) -> f64 {
    1.0 - normalize_value(value, min, max)
}

/// Normalize commission between 0 - 1
fn normalize_commission(commission: u32) -> f64 {
    (commission as f64 / COMMISSION_PLANCK as f64) as f64
}

/// Reverse Normalize commission between 0 - 1
/// lower commission the better
fn reverse_normalize_commission(commission: u32, min: f64, max: f64) -> f64 {
    reverse_normalize_value(
        normalize_commission(commission),
        (min / COMMISSION_PLANCK as f64) as f64,
        (max / COMMISSION_PLANCK as f64) as f64,
    )
}

/// Normalize boolean flag between 0 - 1
fn normalize_flag(flag: bool) -> f64 {
    (flag as u32) as f64
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

async fn _calculate_confidence_interval_95(
    cache: Data<RedisPool>,
    name: &str,
) -> Result<(f64, f64), ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(String, f64)> = redis::cmd("ZRANGE")
        .arg(sync::Key::BoardAtEra(0, name.to_string()))
        .arg("-inf")
        .arg("+inf")
        .arg("BYSCORE")
        .arg("WITHSCORES")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    // Convert Vec<(EraIndex, u32)> to Vec<u32> to easily make the calculation
    let scores: Vec<f64> = v.into_iter().map(|(_, score)| score).collect();
    let min_max = stats::confidence_interval_95(&scores);
    Ok(min_max)
}

async fn calculate_min_max_interval(
    cache: Data<RedisPool>,
    name: &str,
) -> Result<(f64, f64), ApiError> {
    let max = calculate_max_limit(cache.clone(), name).await?;
    let min = calculate_min_limit(cache.clone(), name).await?;
    Ok((min, max))
}

async fn calculate_min_limit(cache: Data<RedisPool>, name: &str) -> Result<f64, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(String, f64)> = redis::cmd("ZRANGE")
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
        return Ok(0.0);
    }
    Ok(v[0].1)
}

async fn calculate_max_limit(cache: Data<RedisPool>, name: &str) -> Result<f64, ApiError> {
    let mut conn = get_conn(&cache).await?;
    let v: Vec<(String, f64)> = redis::cmd("ZRANGE")
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
        return Ok(0.0);
    }
    Ok(v[0].1)
}

async fn cache_board_limits(
    era_index: EraIndex,
    board_name: String,
    cache: Data<RedisPool>,
) -> Result<BoardLimits, ApiError> {
    let mut conn = get_conn(&cache).await?;

    let mut limits: BoardLimitsCache = BTreeMap::new();

    // let max_avg_reward_points =
    //     calculate_avg_points(cache.clone(), sync::BOARD_MAX_POINTS_ERAS).await?;
    // limits.insert("max_avg_reward_points".to_string(), max_avg_reward_points);
    // let min_avg_reward_points =
    //     calculate_avg_points(cache.clone(), sync::BOARD_MIN_POINTS_ERAS).await?;
    // limits.insert("min_avg_reward_points".to_string(), min_avg_reward_points);

    let avg_reward_points_interval =
        calculate_min_max_interval(cache.clone(), sync::BOARD_AVG_POINTS_ERAS).await?;
    limits.insert(
        "min_avg_reward_points".to_string(),
        avg_reward_points_interval.0,
    );
    limits.insert(
        "max_avg_reward_points".to_string(),
        avg_reward_points_interval.1,
    );

    let own_stake_interval =
        calculate_min_max_interval(cache.clone(), sync::BOARD_OWN_STAKE_VALIDATORS).await?;
    // let own_stake_interval = calculate_confidence_interval_95(cache.clone(), sync::BOARD_OWN_STAKE_VALIDATORS).await?;
    limits.insert("min_own_stake".to_string(), own_stake_interval.0);
    limits.insert("max_own_stake".to_string(), own_stake_interval.1);

    let total_stake_interval =
        calculate_min_max_interval(cache.clone(), sync::BOARD_TOTAL_STAKE_VALIDATORS).await?;
    // let total_stake_interval = calculate_confidence_interval_95(cache.clone(), sync::BOARD_TOTAL_STAKE_VALIDATORS).await?;
    limits.insert("min_total_stake".to_string(), total_stake_interval.0);
    limits.insert("max_total_stake".to_string(), total_stake_interval.1);

    let judgements_interval =
        calculate_min_max_interval(cache.clone(), sync::BOARD_JUDGEMENTS_VALIDATORS).await?;
    // let judgements_interval = calculate_confidence_interval_95(cache.clone(), sync::BOARD_JUDGEMENTS_VALIDATORS).await?;
    limits.insert("min_judgements".to_string(), judgements_interval.0);
    limits.insert("max_judgements".to_string(), judgements_interval.1);

    let sub_accounts_interval =
        calculate_min_max_interval(cache.clone(), sync::BOARD_SUB_ACCOUNTS_VALIDATORS).await?;
    // let sub_accounts_interval = calculate_confidence_interval_95(cache.clone(), sync::BOARD_SUB_ACCOUNTS_VALIDATORS).await?;
    limits.insert("min_sub_accounts".to_string(), sub_accounts_interval.0);
    limits.insert("max_sub_accounts".to_string(), sub_accounts_interval.1);

    let key_limits = sync::Key::BoardAtEra(era_index, format!("{}:limits", board_name));
    // Cache board limits
    let _: () = redis::cmd("HSET")
        .arg(key_limits.to_string())
        .arg(limits.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    Ok(limits.into())
}
async fn is_syncing(cache: Data<RedisPool>) -> Result<bool, ApiError> {
    let mut conn = get_conn(&cache).await?;

    let res: Option<String> = redis::cmd("HGET")
        .arg(sync::Key::Info)
        .arg("syncing")
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    let syncing = match res {
        Some(v) => v.parse::<bool>().unwrap_or_default(),
        None => false,
    };

    Ok(syncing)
}

fn calculate_scores(
    validator: &Validator,
    limits: &BoardLimits,
    weights: &Weights,
) -> Result<Vec<f64>, ApiError> {
    let mut scores: Vec<f64> = Vec::with_capacity(WEIGHTS_CAPACITY);

    scores.push(
        normalize_value(
            validator.inclusion_rate as f64,
            limits.inclusion_rate.min,
            limits.inclusion_rate.max,
        ) * weights[0] as f64,
    );
    scores.push(
        reverse_normalize_commission(
            validator.commission,
            limits.commission.min,
            limits.commission.max,
        ) * weights[1] as f64,
    );
    scores.push(
        reverse_normalize_value(
            validator.nominators as f64,
            limits.nominators.min,
            limits.nominators.max,
        ) * weights[2] as f64,
    );
    scores.push(
        normalize_value(
            validator.avg_reward_points,
            limits.avg_reward_points.min,
            limits.avg_reward_points.max,
        ) * weights[3] as f64,
    );
    scores.push(normalize_flag(validator.reward_staked) * weights[4] as f64);
    scores.push(normalize_flag(validator.active) * weights[5] as f64);
    scores.push(
        normalize_value(
            validator.own_stake as f64,
            limits.own_stake.min,
            limits.own_stake.max,
        ) * weights[6] as f64,
    );
    scores.push(
        reverse_normalize_value(
            (validator.own_stake + validator.nominators_stake) as f64,
            limits.total_stake.min,
            limits.total_stake.max,
        ) * weights[7] as f64,
    );
    scores.push(
        normalize_value(
            validator.judgements as f64,
            limits.judgements.min,
            limits.judgements.max,
        ) * weights[8] as f64,
    );
    scores.push(
        reverse_normalize_value(
            validator.sub_accounts as f64,
            limits.sub_accounts.min,
            limits.sub_accounts.max,
        ) * weights[9] as f64,
    );

    Ok(scores)
}

async fn generate_board_scores(
    era_index: EraIndex,
    weights: &Weights,
    cache: Data<RedisPool>,
) -> Result<(), ApiError> {
    let mut conn = get_conn(&cache).await?;

    let board_name = get_board_name(weights, None);
    let key = sync::Key::BoardAtEra(era_index, board_name.clone());

    let exists: bool = redis::cmd("EXISTS")
        .arg(key.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    // If board is already cached do nothing
    if exists {
        return Ok(());
    }

    // Only generate board if cache is not syncing
    if !exists && is_syncing(cache.clone()).await? {
        let msg = format!(
            "The system is currently syncing. Usually doesn't take long 5 - 10min. Please just wait a few minutes before you try again. Thank you.");
        warn!("{}", msg);
        return Err(ApiError::NotFound(msg));
    }

    // Cache board limits based on all validators
    let limits: BoardLimits = cache_board_limits(era_index, board_name.clone(), cache).await?;

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

        // Calculate scores
        let scores = calculate_scores(&validator, &limits, weights)?;
        let score = scores.iter().fold(0.0, |acc, x| acc + x);

        // Cache total score
        let _: () = redis::cmd("ZADD")
            .arg(key.to_string())
            .arg(score) // score
            .arg(stash.to_string()) // member
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;

        let scores_str: String = scores
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if i == 0 {
                    return x.to_string();
                }
                format!(",{}", x)
            })
            .collect();

        // Cache partial scores
        let key_scores = sync::Key::BoardAtEra(era_index, format!("{}:scores", board_name.clone()));
        let _: () = redis::cmd("HSET")
            .arg(key_scores.to_string())
            .arg(stash.to_string())
            .arg(scores_str.to_string())
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
    }

    Ok(())
}

async fn generate_board_filtered_by_intervals(
    era_index: EraIndex,
    weights: &Weights,
    intervals: &Intervals,
    cache: Data<RedisPool>,
) -> Result<(), ApiError> {
    let mut conn = get_conn(&cache).await?;

    let board_name = get_board_name(weights, Some(intervals));
    let key = sync::Key::BoardAtEra(era_index, board_name.clone());

    let exists: bool = redis::cmd("EXISTS")
        .arg(key.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    // If board is already cached do nothing
    // if exists {
    //     return Ok(());
    // }

    // Only generate board if cache is not syncing
    if !exists && is_syncing(cache.clone()).await? {
        let msg = format!(
            "The system is currently syncing. Usually doesn't take long 5 - 10min. Please just wait a few minutes before you try again. Thank you.");
        warn!("{}", msg);
        return Err(ApiError::NotFound(msg));
    }

    let limits: BoardLimits = intervals.into();

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

        // Verify if validator traits are within the respective interval defined by user
        // Position 0 - Higher Inclusion rate is preferrable
        // Position 1 - Lower Commission is preferrable
        // Position 2 - Lower Nominators is preferrable (limit to 256 -> oversubscribed)
        // Position 3 - Higher Reward Points is preferrable
        // Position 4 - If reward is staked is preferrable
        // Position 5 - If in active set is preferrable
        // Position 6 - Higher own stake is preferrable
        // Position 7 - Lower total stake is preferrable
        // Position 8 - Higher number of Reasonable or KnownGood judgements is preferrable
        // Position 9 - Lower number of sub-accounts is preferrable

        if (validator.inclusion_rate as f64) < limits.inclusion_rate.min
            || (validator.inclusion_rate as f64) > limits.inclusion_rate.max
        {
            continue;
        }
        if (validator.commission as f64) < limits.commission.min
            || (validator.commission as f64) > limits.commission.max
        {
            continue;
        }
        if (validator.nominators as f64) < limits.nominators.min
            || ((validator.nominators as f64) > limits.nominators.max
                && limits.nominators.max < NOMINATORS_OVERSUBSCRIBED_THRESHOLD as f64)
        {
            continue;
        }
        if validator.avg_reward_points < limits.avg_reward_points.min
            || validator.avg_reward_points > limits.avg_reward_points.max
        {
            continue;
        }
        if normalize_flag(validator.reward_staked) != limits.reward_staked.min
            && limits.reward_staked.min == limits.reward_staked.max
        {
            continue;
        }
        if normalize_flag(validator.active) != limits.active.min
            && limits.active.min == limits.active.max
        {
            continue;
        }
        if validator.own_stake < limits.own_stake.min as u128
            || validator.own_stake > limits.own_stake.max as u128
        {
            continue;
        }
        if (validator.own_stake + validator.nominators_stake) < limits.total_stake.min as u128
            || (validator.own_stake + validator.nominators_stake) > limits.total_stake.max as u128
        {
            continue;
        }
        if (validator.judgements as f64) < limits.judgements.min
            || (validator.judgements as f64) > limits.judgements.max
        {
            continue;
        }
        if (validator.sub_accounts as f64) < limits.sub_accounts.min
            || (validator.sub_accounts as f64) > limits.sub_accounts.max
        {
            continue;
        }

        // Calculate scores
        let scores = calculate_scores(&validator, &limits, weights)?;
        let score = scores.iter().fold(0.0, |acc, x| acc + x);

        // Cache total score
        let _: () = redis::cmd("ZADD")
            .arg(key.to_string())
            .arg(score) // score
            .arg(stash.to_string()) // member
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;

        let scores_str: String = scores
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if i == 0 {
                    return x.to_string();
                }
                format!(",{}", x)
            })
            .collect();

        // Cache partial scores
        let key_scores = sync::Key::BoardAtEra(era_index, format!("{}:scores", board_name.clone()));
        let _: () = redis::cmd("HSET")
            .arg(key_scores.to_string())
            .arg(stash.to_string())
            .arg(scores_str.to_string())
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
    }

    Ok(())
}

/// Increase board stats counter
async fn increase_board_stats(key: sync::Key, cache: Data<RedisPool>) -> Result<(), ApiError> {
    let mut conn = get_conn(&cache).await?;

    let _: () = redis::cmd("HINCRBY")
        .arg(sync::Key::Stats)
        .arg(key)
        .arg(1)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    Ok(())
}

async fn get_board_limits(
    era_index: EraIndex,
    weights: &Weights,
    cache: Data<RedisPool>,
) -> Result<BoardLimits, ApiError> {
    let mut conn = get_conn(&cache).await?;

    // Check if limits key is already available
    let key = sync::Key::BoardAtEra(
        era_index,
        format!("{}:limits", get_board_name(weights, None)),
    );
    if let redis::Value::Int(0) = redis::cmd("EXISTS")
        .arg(key.clone())
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?
    {
        let msg = format!(
            "Limits not yet available for Leaderboard {:?}. Wait a second and try again.",
            weights
        );
        error!("{}", msg);
        return Err(ApiError::NotFound(msg));
    }
    // Get limits
    let limits: BoardLimitsCache = redis::cmd("HGETALL")
        .arg(key)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    if limits.len() == 0 {
        let msg = format!("Limits not available for Leaderboard {:?}", weights);
        error!("{}", msg);
        return Err(ApiError::InternalServerError(msg));
    }

    Ok(limits.into())
}

async fn get_validators_stashes(
    key: sync::Key,
    n: Quantity,
    cache: Data<RedisPool>,
) -> Result<Vec<String>, ApiError> {
    let mut conn = get_conn(&cache).await?;

    let stashes: Vec<String> = redis::cmd("ZRANGE")
        .arg(key)
        .arg("+inf")
        .arg("0")
        .arg("BYSCORE")
        .arg("REV")
        .arg("LIMIT")
        .arg("0")
        .arg(n)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

    Ok(stashes)
}

/// Get active validators
async fn get_active_validators(
    era_index: EraIndex,
    n: Quantity,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorsResponse>, ApiError> {
    let key = sync::Key::BoardAtEra(era_index, sync::BOARD_ACTIVE_VALIDATORS.to_string());
    respond_json(ValidatorsResponse {
        addresses: get_validators_stashes(key, n, cache).await?,
        meta: MetaResponse::default(),
    })
}

/// Get all validators
async fn get_all_validators(
    era_index: EraIndex,
    n: Quantity,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorsResponse>, ApiError> {
    let key = sync::Key::BoardAtEra(era_index, sync::BOARD_ALL_VALIDATORS.to_string());
    respond_json(ValidatorsResponse {
        addresses: get_validators_stashes(key, n, cache).await?,
        meta: MetaResponse::default(),
    })
}

/// Get board validators
async fn get_board_validators(
    era_index: EraIndex,
    params: Query<Params>,
    cache: Data<RedisPool>,
) -> Result<Json<ValidatorsResponse>, ApiError> {
    let key = sync::Key::BoardAtEra(era_index, get_board_name(&params.w, Some(&params.i)));

    // Generate leaderboard scores and cache it
    generate_board_scores(era_index, &params.w, cache.clone()).await?;

    // Generate filtered leaderboard and cache it
    generate_board_filtered_by_intervals(era_index, &params.w, &params.i, cache.clone()).await?;

    // Increase board stats counter
    increase_board_stats(key.clone(), cache.clone()).await?;

    let limits: BoardLimits = get_board_limits(era_index, &params.w, cache.clone()).await?;

    respond_json(ValidatorsResponse {
        addresses: get_validators_stashes(key.clone(), params.n, cache.clone()).await?,
        meta: MetaResponse {
            limits: limits.to_string(),
        },
    })
}

/// Get validators
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

    match params.q {
        Queries::Active => {
            return get_active_validators(era_index, params.n, cache).await;
        }
        Queries::All => {
            return get_all_validators(era_index, params.n, cache).await;
        }
        Queries::Board => {
            return get_board_validators(era_index, params, cache).await;
        }
        _ => {
            let msg = format!(
                "Parameter q={} must be equal to one of the options: [Active, All, Board]",
                params.q
            );
            warn!("{}", msg);
            return Err(ApiError::BadRequest(msg));
        }
    }
}
