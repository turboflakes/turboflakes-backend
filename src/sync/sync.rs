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

use crate::cache::{create_pool, RedisPool};
use crate::config::{Config, CONFIG};
use crate::errors::{CacheError, SyncError};
use crate::sync::stats::{max, mean, median, min};
use async_recursion::async_recursion;
use async_std::task;
use chrono::Utc;
use codec::Encode;
use log::{debug, error, info};
use redis::aio::Connection;
use regex::Regex;
use std::{collections::BTreeMap, convert::TryInto, marker::PhantomData, result::Result};
use std::{thread, time};
use substrate_subxt::{
  identity::{IdentityOfStoreExt, SuperOfStoreExt},
  session::ValidatorsStore,
  sp_core::storage::StorageKey,
  sp_core::Decode,
  sp_runtime::AccountId32,
  staking::{
    ActiveEraStoreExt, BondedStoreExt, EraIndex, EraPayoutEvent, ErasRewardPointsStoreExt,
    ErasStakersClippedStoreExt, ErasStakersStoreExt, ErasTotalStakeStoreExt,
    ErasValidatorPrefsStoreExt, ErasValidatorRewardStoreExt, HistoryDepthStoreExt, PayeeStoreExt,
    RewardDestination, RewardPoint, ValidatorsStoreExt,
  },
  Client, ClientBuilder, DefaultNodeRuntime, EventSubscription,
};

pub async fn create_substrate_node_client(
  config: Config,
) -> Result<Client<DefaultNodeRuntime>, substrate_subxt::Error> {
  ClientBuilder::<DefaultNodeRuntime>::new()
    .set_url(config.substrate_ws_url)
    .skip_type_sizes_check()
    .build()
    .await
}

fn get_account_id_from_storage_key(key: StorageKey) -> AccountId32 {
  let s = &key.0[key.0.len() - 32..];
  let v: [u8; 32] = s.try_into().expect("slice with incorrect length");
  AccountId32::new(v)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
  ActiveEra,
  Era(EraIndex),
  ValidatorAtEra(EraIndex, AccountId32),
  BoardAtEra(EraIndex, String),
  ValidatorAtEraScan(AccountId32),
  Validator(AccountId32),
  ActiveErasByValidator(AccountId32),
}

impl std::fmt::Display for Key {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ActiveEra => write!(f, "era:active"),
      Self::Era(era_index) => write!(f, "{}:era", era_index),
      Self::ValidatorAtEra(era_index, stash_account) => {
        write!(f, "{}:era:{}:val", era_index, stash_account)
      }
      Self::BoardAtEra(era_index, name) => write!(f, "{}:era:{}:board", era_index, name),
      Self::ValidatorAtEraScan(stash_account) => write!(f, "*:era:{}:val", stash_account),
      Self::Validator(stash_account) => write!(f, "{}:val", stash_account),
      Self::ActiveErasByValidator(stash_account) => write!(f, "{}:val:eras:active", stash_account),
    }
  }
}

impl redis::ToRedisArgs for Key {
  fn write_redis_args<W>(&self, out: &mut W)
  where
    W: ?Sized + redis::RedisWrite,
  {
    out.write_arg(self.to_string().as_bytes())
  }
}

pub struct Sync {
  pub cache_pool: RedisPool,
  pub node_client: substrate_subxt::Client<DefaultNodeRuntime>,
}

impl Sync {
  pub async fn new() -> Sync {
    let cache_pool = create_pool(CONFIG.clone()).expect("failed to create Redis pool");
    let node_client = create_substrate_node_client(CONFIG.clone())
      .await
      .expect("failed to get substrate node connection");
    Sync {
      cache_pool: cache_pool,
      node_client: node_client,
    }
  }

  async fn check_cache(&self) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    let pong: String = redis::cmd("PING")
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    if pong.as_str() != "PONG" {
      return Err(CacheError::RedisPongError.into());
    }
    Ok(())
  }

  async fn ready_or_await(&self) {
    while let Err(e) = self.check_cache().await {
      error!("{}", e);
      info!("Awaiting for Redis to be ready");
      thread::sleep(time::Duration::from_secs(6));
    }
  }

  async fn history(&self) -> Result<(), SyncError> {
    self.ready_or_await().await;

    let active_era = self.active_era().await?;

    self.eras_history_depth(active_era).await?;

    self.validators().await?;

    self.active_validators().await?;

    Ok(())
  }

  async fn subscribe(&self) -> Result<(), SyncError> {
    info!("Starting subscription");
    self.ready_or_await().await;
    let client = self.node_client.clone();
    let sub = client.subscribe_finalized_events().await?;
    let decoder = client.events_decoder();
    let mut sub = EventSubscription::<DefaultNodeRuntime>::new(sub, decoder);
    sub.filter_event::<EraPayoutEvent<_>>();
    info!("Waiting for EraPayoutEvent");
    while let Some(result) = sub.next().await {
      if let Ok(raw_event) = result {
        match EraPayoutEvent::<DefaultNodeRuntime>::decode(&mut &raw_event.data[..]) {
          Ok(event) => {
            info!("Successfully decoded event {:?}", event);
            self.active_era().await?;
            self.eras_history(event.era_index, Some(true)).await?;
            self.validators().await?;
            self.active_validators().await?;
          }
          Err(e) => {
            error!("Decoding event error: {:?}", e);
          }
        }
      }
    }
    // If subscription has closed for some reason await and subscribe again
    Err(SyncError::SubscriptionFinished)
  }

  /// Spawn history and subscription sincronization tasks
  pub fn run() {
    spawn_and_restart_history_on_error();
    spawn_and_restart_subscription_on_error();
  }

  /// Sync active era
  async fn active_era(&self) -> Result<EraIndex, SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();
    let active_era = client.active_era(None).await?;

    let _: () = redis::cmd("SET")
      .arg(Key::ActiveEra)
      .arg(active_era.index)
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    info!("Successfully synced active era {}", active_era.index);
    Ok(active_era.index)
  }

  /// Sync all validators currently available
  async fn validators(&self) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    let history_depth: u32 = client.history_depth(None).await?;
    let active_era = client.active_era(None).await?;
    let mut validators = client.validators_iter(None).await?;
    while let Some((key, validator_prefs)) = validators.next().await? {
      let mut validator_data: BTreeMap<String, String> = BTreeMap::new();
      validator_data.insert(
        "commission".to_string(),
        validator_prefs.commission.deconstruct().to_string(),
      );
      validator_data.insert("blocked".to_string(), validator_prefs.blocked.to_string());

      let stash = get_account_id_from_storage_key(key);
      // Sync controller
      let controller = match client.bonded(stash.clone(), None).await? {
        Some(c) => c,
        None => {
          return Err(SyncError::Other(format!(
            "Controller account not found for stash {:?}",
            stash
          )))
        }
      };
      validator_data.insert("controller".to_string(), controller.to_string());

      // Sync payee - where the reward payment should be made
      let reward_staked = if RewardDestination::Staked == client.payee(stash.clone(), None).await? {
        true
      } else {
        false
      };
      validator_data.insert("reward_staked".to_string(), reward_staked.to_string());

      // Calculate inclusion rate
      let inclusion_rate = self
        .calculate_inclusion_rate(&stash, active_era.index - history_depth, active_era.index)
        .await?;
      validator_data.insert("inclusion_rate".to_string(), inclusion_rate.to_string());

      // Calculate mean reward points
      let mean_reward_points = self
        .calculate_mean_reward_points(&stash, active_era.index - history_depth, active_era.index)
        .await?;
      validator_data.insert(
        "mean_reward_points".to_string(),
        mean_reward_points.to_string(),
      );

      // Fetch identity
      let name = self.get_identity(&stash, None).await?;
      validator_data.insert("name".to_string(), name.to_string());

      self
        .set_eras_validator_stakers(active_era.index, &stash, &mut validator_data)
        .await?;

      self
        .set_eras_validator_stakers_clipped(active_era.index, &stash, &mut validator_data)
        .await?;

      // Cache information for the stash
      let _: () = redis::cmd("HSET")
        .arg(Key::Validator(stash.clone()))
        .arg(validator_data)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      // Add stash to the sorted set board named: all
      let _: () = redis::cmd("ZADD")
        .arg(Key::BoardAtEra(active_era.index, "all".to_string()))
        .arg(0) // score
        .arg(stash.to_string()) // member
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      debug!("Successfully synced validator with stash {}", stash);
    }

    info!(
      "Successfully synced all validators in era {}",
      active_era.index
    );

    Ok(())
  }

  #[async_recursion]
  async fn get_identity(
    &self,
    stash: &AccountId32,
    sub_account_name: Option<String>,
  ) -> Result<String, SyncError> {
    let client = self.node_client.clone();

    let re = Regex::new(r"[\r\n\f]+").unwrap();
    let display: String = match client.identity_of(stash.clone(), None).await? {
      Some(registration) => {
        let mut parent =
          String::from_utf8(registration.info.display.encode()).unwrap_or("-".to_string());
        parent = re.replace_all(&parent, "").to_string();
        if let Some(n) = sub_account_name {
          format!("{}/{}", parent, n)
        } else {
          parent
        }
      }
      None => {
        if let Some((parent_account, data)) = client.super_of(stash.clone(), None).await? {
          let mut sub_account_name = String::from_utf8(data.encode()).unwrap();
          sub_account_name = re.replace_all(&sub_account_name, "").to_string();
          return self
            .get_identity(&parent_account, Some(sub_account_name))
            .await;
        } else {
          "".to_string()
        }
      }
    };
    Ok(display.to_string())
  }

  /// Calculate inclusion rate for the last depth history eras
  async fn calculate_inclusion_rate(
    &self,
    stash: &AccountId32,
    era_index_min: EraIndex,
    era_index_max: EraIndex,
  ) -> Result<f32, SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    let count: f32 = redis::cmd("ZCOUNT")
      .arg(Key::ActiveErasByValidator(stash.clone()))
      .arg(format!("{}", era_index_min))
      .arg(format!("({}", era_index_max))
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    let inclusion = count / (era_index_max as f32 - era_index_min as f32);

    Ok(inclusion)
  }

  /// Calculate mean reward points for all eras available
  async fn calculate_mean_reward_points(
    &self,
    stash: &AccountId32,
    era_index_min: EraIndex,
    era_index_max: EraIndex,
  ) -> Result<f64, SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    // Get range of members in the sorted set between specific eras
    // the era format is currently defined by era:points
    let t: Vec<String> = redis::cmd("ZRANGE")
      .arg(Key::ActiveErasByValidator(stash.clone()))
      .arg(format!("{}", era_index_min))
      .arg(format!("({}", era_index_max))
      .arg("BYSCORE")
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    // To easily calculate the mean we first convert the members Vector to a points Vector
    // [era1:points1, era2:points2, ..] -> [points1, points2, ..]
    let v: Vec<u32> = t
      .into_iter()
      .map(|x| {
        let i = x.find(':').unwrap();
        let points: u32 = String::from(&x[i + 1..x.len()]).parse().unwrap();
        points
      })
      .collect();

    let mean = mean(&v);

    Ok(mean)
  }

  /// Sync active validators for specific era
  async fn active_validators(&self) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    let active_era = client.active_era(None).await?;
    let store = ValidatorsStore {
      _runtime: PhantomData,
    };
    let result = client.fetch(&store, None).await?;
    let validators = match result {
      Some(v) => v,
      None => {
        println!("No Validators available in the active era");
        vec![]
      }
    };
    for stash in validators.iter() {
      // Cache information for the stash
      let _: () = redis::cmd("HSET")
        .arg(Key::Validator(stash.clone()))
        .arg(&[("active", "true")])
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      // Add stash to the sorted set board named: active
      let _: () = redis::cmd("ZADD")
        .arg(Key::BoardAtEra(active_era.index, "active".to_string()))
        .arg(0) // score
        .arg(stash.to_string()) // member
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    }

    info!(
      "Successfully synced {} active validators in era {}",
      &validators.len(),
      active_era.index
    );

    Ok(())
  }

  /// Sync all era information for all history depth
  async fn eras_history_depth(&self, active_era_index: EraIndex) -> Result<(), SyncError> {
    let client = self.node_client.clone();

    let history_depth: u32 = client.history_depth(None).await?;
    let start_index = active_era_index - history_depth;
    for era_index in start_index..active_era_index {
      self.eras_history(era_index, None).await?;
    }
    info!("Successfully synced {} eras history", history_depth);

    Ok(())
  }

  /// Sync all era information for a given era.
  ///
  /// <ErasValidatorReward<T>>;       --> collected
  /// <ErasRewardPoints<T>>;          --> collected
  /// <ErasTotalStake<T>>;            --> collected
  /// ErasStartSessionIndex;          --> not needed for now
  #[async_recursion]
  async fn eras_history(&self, era_index: EraIndex, force: Option<bool>) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    if let Some(true) = force {
      self.eras_validator_reward(era_index).await?;
      self.eras_total_stake(era_index).await?;
      self.eras_reward_points(era_index).await?;
      let _: () = redis::cmd("HSET")
        .arg(Key::Era(era_index))
        .arg(&[("synced_at", Utc::now().timestamp().to_string())])
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
      info!("Successfully synced history in era {}", era_index);

      return Ok(());
    }
    // Check if era is already synced
    let is_synced: bool = redis::cmd("HEXISTS")
      .arg(Key::Era(era_index))
      .arg("synced_at")
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    if is_synced {
      info!("Skipping era {} -> already synced", era_index);
      return Ok(());
    }

    return self.eras_history(era_index, Some(true)).await;
  }

  /// Sync <ErasValidatorReward<T>>
  async fn eras_validator_reward(&self, era_index: EraIndex) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    let result = client.eras_validator_reward(era_index, None).await?;
    let reward = match result {
      Some(v) => v,
      None => 0,
    };

    let _: () = redis::cmd("HSET")
      .arg(Key::Era(era_index))
      .arg(&[("total_reward", reward.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    debug!("Successfully synced total rewards in era {}", era_index);
    Ok(())
  }

  /// Sync <ErasTotalStake<T>>
  async fn eras_total_stake(&self, era_index: EraIndex) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    let total_stake = client.eras_total_stake(era_index, None).await?;
    let _: () = redis::cmd("HSET")
      .arg(Key::Era(era_index))
      .arg(&[("total_stake", total_stake.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    debug!("Successfully synced total stake in era {}", era_index);

    Ok(())
  }

  /// Sync <ErasRewardPoints<T>>;
  async fn eras_reward_points(&self, era_index: EraIndex) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    let era_reward_points = client.eras_reward_points(era_index, None).await?;
    let mut reward_points: Vec<RewardPoint> =
      Vec::with_capacity(era_reward_points.individual.len());
    for (stash, points) in era_reward_points.individual.iter() {
      reward_points.push(*points);
      let mut validator_data: BTreeMap<String, String> = BTreeMap::new();
      validator_data.insert("active".to_string(), "true".to_string());
      validator_data.insert("reward_points".to_string(), points.to_string());

      self
        .set_eras_validator_prefs(era_index, stash, &mut validator_data)
        .await?;

      self
        .set_eras_validator_stakers(era_index, stash, &mut validator_data)
        .await?;

      self
        .set_eras_validator_stakers_clipped(era_index, stash, &mut validator_data)
        .await?;

      let _: () = redis::cmd("HSET")
        .arg(Key::ValidatorAtEra(era_index, stash.clone()))
        .arg(validator_data)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      let member = format!("{}:{}", era_index, points);
      let _: () = redis::cmd("ZADD")
        .arg(Key::ActiveErasByValidator(stash.clone()))
        .arg(era_index) // score
        .arg(member) // member
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      debug!(
        "Successfully synced validator reward points with stash {} in era {}",
        stash, era_index
      );
    }
    let _: () = redis::cmd("HSET")
      .arg(Key::Era(era_index))
      .arg(&[
        ("total_reward_points", era_reward_points.total.to_string()),
        ("min_reward_points", min(&reward_points).to_string()),
        ("max_reward_points", max(&reward_points).to_string()),
        ("mean_reward_points", mean(&reward_points).to_string()),
        (
          "median_reward_points",
          median(&mut reward_points).to_string(),
        ),
      ])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    debug!(
      "Successfully synced total reward points in era {}",
      era_index
    );

    Ok(())
  }

  /// Sync <ErasValidatorPrefs<T>>;
  async fn set_eras_validator_prefs<'a>(
    &self,
    era_index: EraIndex,
    stash: &AccountId32,
    data: &'a mut BTreeMap<String, String>,
  ) -> Result<(), SyncError> {
    let client = self.node_client.clone();

    let stash_cloned = stash.clone();
    let validator_prefs = client
      .eras_validator_prefs(era_index, stash_cloned, None)
      .await?;
    data.insert(
      "commission".to_string(),
      validator_prefs.commission.deconstruct().to_string(),
    );
    data.insert("blocked".to_string(), validator_prefs.blocked.to_string());

    debug!(
      "Successfully synced validator prefs with stash {} in era {}",
      stash, era_index
    );
    Ok(())
  }

  /// Sync <ErasStakers<T>>;
  async fn set_eras_validator_stakers<'a>(
    &self,
    era_index: EraIndex,
    stash: &AccountId32,
    data: &'a mut BTreeMap<String, String>,
  ) -> Result<(), SyncError> {
    let client = self.node_client.clone();

    let exposure = client.eras_stakers(era_index, stash.clone(), None).await?;
    let mut others_stake: u128 = 0;
    for individual_exposure in exposure.others.iter() {
      others_stake += individual_exposure.value;
    }
    data.insert("total_stake".to_string(), exposure.total.to_string());
    data.insert("own_stake".to_string(), exposure.own.to_string());
    data.insert("others_stake".to_string(), others_stake.to_string());
    data.insert("stakers".to_string(), exposure.others.len().to_string());

    debug!(
      "Successfully synced validator total stake with stash {} in era {}",
      stash, era_index
    );

    Ok(())
  }

  /// Sync <ErasStakersClipped<T>>;
  async fn set_eras_validator_stakers_clipped<'a>(
    &self,
    era_index: EraIndex,
    stash: &AccountId32,
    data: &'a mut BTreeMap<String, String>,
  ) -> Result<(), SyncError> {
    let client = self.node_client.clone();

    let exposure = client
      .eras_stakers_clipped(era_index, stash.clone(), None)
      .await?;
    let mut others_stake: u128 = 0;
    for individual_exposure in exposure.others.iter() {
      others_stake += individual_exposure.value;
    }
    data.insert("others_stake_clipped".to_string(), others_stake.to_string());
    data.insert(
      "stakers_clipped".to_string(),
      exposure.others.len().to_string(),
    );

    debug!(
      "Successfully synced validator clipped stake with stash {} in era {}",
      stash, era_index
    );

    Ok(())
  }
}

pub fn spawn_and_restart_subscription_on_error() {
  task::spawn(async {
    loop {
      let sync: Sync = Sync::new().await;
      if let Err(e) = sync.subscribe().await {
        error!("{}", e);
        thread::sleep(time::Duration::from_millis(500));
      };
    }
  });
}

pub fn spawn_and_restart_history_on_error() {
  task::spawn(async {
    loop {
      let sync: Sync = Sync::new().await;
      match sync.history().await {
        Ok(()) => break,
        Err(e) => {
          error!("{}", e);
          thread::sleep(time::Duration::from_millis(1000));
        }
      }
    }
  });
}
