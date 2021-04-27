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
use chrono::Utc;
use codec::Encode;
use log::{debug, error, info};
use redis::aio::Connection;
use regex::Regex;
use std::{collections::BTreeMap, convert::TryInto, marker::PhantomData, result::Result};
use substrate_subxt::{
  identity::{IdentityOfStoreExt, SuperOfStoreExt},
  session::ValidatorsStore,
  sp_core::storage::StorageKey,
  sp_core::Decode,
  sp_runtime::AccountId32,
  staking::{
    ActiveEraStoreExt, BondedStoreExt, EraIndex, EraPayoutEvent, ErasRewardPointsStoreExt,
    ErasStakersStoreExt, ErasTotalStakeStoreExt, ErasValidatorPrefsStoreExt,
    ErasValidatorRewardStoreExt, HistoryDepthStoreExt, PayeeStoreExt, RewardDestination,
    RewardPoint, ValidatorsStoreExt,
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

  async fn check(&self) -> Result<(), SyncError> {
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

  pub async fn run(&self) -> Result<(), SyncError> {
    self.check().await?;

    let active_era = self.active_era().await?;

    self.eras_history_depth(active_era).await?;

    self.validators().await?;

    self.active_validators().await?;

    Ok(())
  }

  pub async fn subscribe(&self) -> Result<(), SyncError> {
    self.check().await?;

    let client = self.node_client.clone();
    let sub = client.subscribe_finalized_events().await?;
    let decoder = client.events_decoder();
    let mut sub = EventSubscription::<DefaultNodeRuntime>::new(sub, decoder);
    sub.filter_event::<EraPayoutEvent<_>>();
    loop {
      if let Some(result) = sub.next().await {
        match result {
          Ok(raw_event) => {
            match EraPayoutEvent::<DefaultNodeRuntime>::decode(&mut &raw_event.data[..]) {
              Ok(event) => {
                info!("successfully decoded event {:?}", event);
                self.active_era().await?;
                self.eras_history(event.era_index, Some(true)).await?;
                self.validators().await?;
                self.active_validators().await?;
              }
              Err(e) => {
                error!("decoding event error: {:?}", e);
              }
            }
          }
          Err(e) => {
            error!("subscription event error: {:?}", e);
          }
        }
      }
    }

    // Ok(())
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
    let key = format!("era:active");
    let _: () = redis::cmd("SET")
      .arg(key)
      .arg(active_era.index)
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    info!("successfully synced active era {}", active_era.index);
    Ok(active_era.index)
  }

  async fn add_stash_to_era(
    &self,
    stash: &AccountId32,
    era_index: EraIndex,
    is_active: bool,
  ) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    let end = if is_active { ":active" } else { "" };

    // add stash to the list of validators available in era
    let key = format!("{}:era:vals{}", era_index, end);
    let _: () = redis::cmd("SADD")
      .arg(key)
      .arg(stash.to_string())
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    Ok(())
  }

  async fn add_era_reward_points_to_stash(
    &self,
    stash: &AccountId32,
    era_index: EraIndex,
    reward_points: u32,
  ) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    // add era index to the list of eras where stash was active
    let key = format!("{}:val:eras:active", stash);
    let _: () = redis::cmd("ZADD")
      .arg(key)
      .arg(reward_points)
      .arg(era_index)
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    Ok(())
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

      // Sync payee - where the reward payment should be made
      let reward_staked = if RewardDestination::Staked == client.payee(stash.clone(), None).await? {
        true
      } else {
        false
      };

      // Calculate inclusion rate
      let inclusion_rate = self
        .calculate_inclusion_rate(&stash, active_era.index - history_depth, active_era.index)
        .await?;

      // Calculate mean reward points
      let mean_reward_points = self.calculate_mean_reward_points(&stash).await?;

      // Fetch identity
      let name = self.get_identity(&stash, None).await?;
      // Cache information for the stash
      let key = format!("{}:val", stash);
      let _: () = redis::cmd("HSET")
        .arg(key)
        .arg(&[
          ("controller", controller.to_string()),
          ("name", name.to_string()),
          ("inclusion_rate", inclusion_rate.to_string()),
          ("mean_reward_points", mean_reward_points.to_string()),
          ("reward_staked", reward_staked.to_string()),
          (
            "commission",
            validator_prefs.commission.deconstruct().to_string(),
          ),
          ("blocked", validator_prefs.blocked.to_string()),
        ])
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      self
        .add_stash_to_era(&stash, active_era.index, false)
        .await?;

      debug!("successfully synced validator with stash {}", stash);
    }

    info!(
      "successfully synced all validators in era {}",
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

    let key = format!("{}:val:eras:active", stash);
    let count: f32 = redis::cmd("ZLEXCOUNT")
      .arg(key)
      .arg(format!("({}", era_index_min))
      .arg(format!("[{}", era_index_max))
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    Ok(count / (era_index_max as f32 - era_index_min as f32))
  }

  /// Calculate mean reward points for all eras available
  async fn calculate_mean_reward_points(&self, stash: &AccountId32) -> Result<f64, SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    let key = format!("{}:val:eras:active", stash);
    let t: Vec<(u32, u32)> = redis::cmd("ZRANGE")
      .arg(key)
      .arg("0")
      .arg("-1")
      .arg("WITHSCORES")
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    let v: Vec<u32> = t.into_iter().map(|(_era, points)| points).collect();
    Ok(mean(&v))
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
      self
        .add_stash_to_era(&stash, active_era.index, true)
        .await?;

      // Cache information for the stash
      let key = format!("{}:val", stash);
      let _: () = redis::cmd("HSET")
        .arg(key)
        .arg(&[("active", "true")])
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
    }

    info!(
      "successfully synced {} active validators in era {}",
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
    info!("successfully synced {} eras history", history_depth);

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
      let key = format!("{}:era", era_index);
      let _: () = redis::cmd("HSET")
        .arg(key)
        .arg(&[("synced_at", Utc::now().timestamp().to_string())])
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;
      info!("successfully synced history in era {}", era_index);

      return Ok(());
    }
    // Check if era is already synced
    let key = format!("{}:era", era_index);
    let is_synced: bool = redis::cmd("HEXISTS")
      .arg(key)
      .arg("synced_at")
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    if is_synced {
      info!("skipping era {} -> already synced", era_index);
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
    let key = format!("{}:era", era_index);
    let _: () = redis::cmd("HSET")
      .arg(key)
      .arg(&[("total_reward", reward.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    debug!("successfully synced total rewards in era {}", era_index);
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
    let key = format!("{}:era", era_index);
    let _: () = redis::cmd("HSET")
      .arg(key)
      .arg(&[("total_stake", total_stake.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    debug!("successfully synced total stake in era {}", era_index);

    Ok(())
  }

  /// Sync <ErasRewardPoints<T>>;
  pub async fn eras_reward_points(&self, era_index: EraIndex) -> Result<(), SyncError> {
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
      validator_data.insert(String::from("active"), String::from("true"));
      validator_data.insert(String::from("reward_points"), points.to_string());
      self
        .set_eras_validator_prefs(era_index, stash, &mut validator_data)
        .await?;
      self
        .set_eras_validator_stakers(era_index, stash, &mut validator_data)
        .await?;
      let key = format!("{}:era:{}:val", era_index, stash);
      let _: () = redis::cmd("HSET")
        .arg(key)
        .arg(validator_data)
        .query_async(&mut conn as &mut Connection)
        .await
        .map_err(CacheError::RedisCMDError)?;

      self
        .add_era_reward_points_to_stash(stash, era_index, *points)
        .await?;
      debug!(
        "successfully synced validator reward points with stash {} in era {}",
        stash, era_index
      );
    }
    let key = format!("{}:era", era_index);
    let _: () = redis::cmd("HSET")
      .arg(key)
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
      "successfully synced total reward points in era {}",
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
      String::from("commission"),
      validator_prefs.commission.deconstruct().to_string(),
    );
    data.insert(String::from("blocked"), validator_prefs.blocked.to_string());

    debug!(
      "successfully synced validator prefs with stash {} in era {}",
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

    let stash_cloned = stash.clone();
    let exposure = client.eras_stakers(era_index, stash_cloned, None).await?;
    let mut others_stake: u128 = 0;
    for individual_exposure in exposure.others.iter() {
      others_stake += individual_exposure.value;
    }
    data.insert(String::from("total_stake"), exposure.total.to_string());
    data.insert(String::from("own_stake"), exposure.own.to_string());
    data.insert(String::from("others_stake"), others_stake.to_string());

    debug!(
      "successfully synced validator total stake with stash {} in era {}",
      stash, era_index
    );

    Ok(())
  }
}
