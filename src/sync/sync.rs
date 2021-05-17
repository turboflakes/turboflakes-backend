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
use log::{debug, error, info, warn};
use redis::aio::Connection;
use regex::Regex;
use std::{collections::BTreeMap, convert::TryInto, marker::PhantomData, result::Result};
use std::{thread, time};
use substrate_subxt::{
  identity::{IdentityOfStoreExt, Judgement, SubsOfStoreExt, SuperOfStoreExt},
  session::{NewSessionEvent, ValidatorsStore},
  sp_core::storage::StorageKey,
  sp_core::Decode,
  sp_runtime::AccountId32,
  staking::{
    ActiveEraStoreExt, BondedStoreExt, EraIndex, EraPayoutEvent, ErasRewardPointsStoreExt,
    ErasStakersClippedStoreExt, ErasStakersStoreExt, ErasTotalStakeStoreExt,
    ErasValidatorPrefsStoreExt, ErasValidatorRewardStoreExt, HistoryDepthStoreExt, LedgerStoreExt,
    NominatorsStoreExt, PayeeStoreExt, RewardDestination, RewardPoint, ValidatorsStoreExt,
  },
  Client, ClientBuilder, DefaultNodeRuntime, EventSubscription,
};

pub const BOARD_ACTIVE_VALIDATORS: &'static str = "active:val";
pub const BOARD_ALL_VALIDATORS: &'static str = "all:val";
pub const BOARD_TOTAL_POINTS_ERAS: &'static str = "total:points:era";
pub const BOARD_MAX_POINTS_ERAS: &'static str = "max:points:era";
pub const BOARD_MIN_POINTS_ERAS: &'static str = "min:points:era";
pub const BOARD_OWN_STAKE_VALIDATORS: &'static str = "own:stake:val";
pub const BOARD_JUDGEMENTS_VALIDATORS: &'static str = "judgements:val";
pub const BOARD_SUB_ACCOUNTS_VALIDATORS: &'static str = "sub:accounts:val";

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
  Info,
  Stats,
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
      Self::Info => write!(f, "info"),
      Self::Stats => write!(f, "stats"),
      Self::ActiveEra => write!(f, "era:active"),
      Self::Era(era_index) => write!(f, "{}:era", era_index),
      Self::ValidatorAtEra(era_index, stash_account) => {
        write!(f, "{}:era:{}:val", era_index, stash_account)
      }
      Self::ValidatorAtEraScan(stash_account) => write!(f, "*:era:{}:val", stash_account),
      Self::BoardAtEra(era_index, name) => write!(f, "{}:era:{}:board", era_index, name),
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

pub enum Status {
  Started = 1,
  Finished = 2,
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

    self.status(Status::Started).await?;

    let active_era = self.active_era().await?;

    self.eras_history_depth(active_era).await?;

    self.validators().await?;

    self.nominators().await?;

    self.active_validators().await?;

    self.status(Status::Finished).await?;

    Ok(())
  }

  /// Sync previous era history every era payout
  async fn subscribe_era_payout_events(&self) -> Result<(), SyncError> {
    info!("Starting era payout subscription");
    self.ready_or_await().await;
    let client = self.node_client.clone();
    let sub = client.subscribe_finalized_events().await?;
    let decoder = client.events_decoder();
    let mut sub = EventSubscription::<DefaultNodeRuntime>::new(sub, decoder);
    sub.filter_event::<EraPayoutEvent<_>>();
    info!("Waiting for EraPayout events");
    while let Some(result) = sub.next().await {
      if let Ok(raw_event) = result {
        match EraPayoutEvent::<DefaultNodeRuntime>::decode(&mut &raw_event.data[..]) {
          Ok(event) => {
            info!("Successfully decoded event {:?}", event);
            self.status(Status::Started).await?;
            self.active_era().await?;
            self.eras_history(event.era_index, Some(true)).await?;
            self.validators().await?;
            self.active_validators().await?;
            self.nominators().await?;
            self.status(Status::Finished).await?;
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

  /// Sync all validators and nominators every session
  #[allow(dead_code)]
  async fn subscribe_new_session_events(&self) -> Result<(), SyncError> {
    info!("Starting new session subscription");
    self.ready_or_await().await;
    let client = self.node_client.clone();
    let sub = client.subscribe_finalized_events().await?;
    let decoder = client.events_decoder();
    let mut sub = EventSubscription::<DefaultNodeRuntime>::new(sub, decoder);
    sub.filter_event::<NewSessionEvent<_>>();
    info!("Waiting for NewSession events");
    while let Some(result) = sub.next().await {
      if let Ok(raw_event) = result {
        match NewSessionEvent::<DefaultNodeRuntime>::decode(&mut &raw_event.data[..]) {
          Ok(event) => {
            info!("Successfully decoded event {:?}", event);
            self.status(Status::Started).await?;
            self.validators().await?;
            self.nominators().await?;
            self.status(Status::Finished).await?;
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
    // Note: Just make a full sync every era payout event
    spawn_and_restart_era_payout_subscription_on_error();
    // TODO: Single track events based on the feature that got changed
    // spawn_and_restart_new_session_subscription_on_error();
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

  /// Cache syncronization status
  async fn status(&self, status: Status) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;

    let mut data: BTreeMap<String, String> = BTreeMap::new();
    match status {
      Status::Started => {
        data.insert("syncing".to_string(), "true".to_string());
        data.insert(
          "syncing_started_at".to_string(),
          Utc::now().timestamp().to_string(),
        );
      }
      Status::Finished => {
        data.insert("syncing".to_string(), "false".to_string());
        data.insert(
          "syncing_finished_at".to_string(),
          Utc::now().timestamp().to_string(),
        );
      }
    }

    let _: () = redis::cmd("HSET")
      .arg(Key::Info)
      .arg(data)
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

    info!("Starting validators sync");
    let history_depth: u32 = client.history_depth(None).await?;
    let active_era = client.active_era(None).await?;
    let mut validators = client.validators_iter(None).await?;
    let mut i: u32 = 0;
    while let Some((key, validator_prefs)) = validators.next().await? {
      let stash = get_account_id_from_storage_key(key);
      // Sync controller
      if let Some(controller) = client.bonded(stash.clone(), None).await? {
        let mut validator_data: BTreeMap<String, String> = BTreeMap::new();
        validator_data.insert("active".to_string(), "false".to_string());
        validator_data.insert(
          "commission".to_string(),
          validator_prefs.commission.deconstruct().to_string(),
        );
        validator_data.insert("blocked".to_string(), validator_prefs.blocked.to_string());

        validator_data.insert("controller".to_string(), controller.to_string());
        // Fetch own stake
        let own_stake = self.get_controller_stake(&controller).await?;
        validator_data.insert("own_stake".to_string(), own_stake.to_string());
        if own_stake != 0 {
          let _: () = redis::cmd("ZADD")
            .arg(Key::BoardAtEra(0, BOARD_OWN_STAKE_VALIDATORS.to_string()))
            .arg(own_stake.to_string()) // score
            .arg(stash.to_string()) // member
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
        }
        // Sync payee - where the reward payment should be made
        let reward_staked =
          if RewardDestination::Staked == client.payee(stash.clone(), None).await? {
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

        // Calculate average reward points
        let avg_reward_points = self
          .calculate_avg_reward_points(&stash, active_era.index - history_depth, active_era.index)
          .await?;
        validator_data.insert(
          "avg_reward_points".to_string(),
          avg_reward_points.to_string(),
        );

        // Fetch identity
        let mut identity_data = self.get_identity(&stash, None).await?;
        validator_data.append(&mut identity_data);

        // NOTE: Reset nominators counters
        validator_data.insert("nominators".to_string(), "0".to_string());
        validator_data.insert("nominators_stake".to_string(), "0".to_string());

        // Cache information for the stash
        let _: () = redis::cmd("HSET")
          .arg(Key::Validator(stash.clone()))
          .arg(validator_data.clone())
          .query_async(&mut conn as &mut Connection)
          .await
          .map_err(CacheError::RedisCMDError)?;

        // Add stash to the sorted set board named: all
        let _: () = redis::cmd("ZADD")
          .arg(Key::BoardAtEra(
            active_era.index,
            BOARD_ALL_VALIDATORS.to_string(),
          ))
          .arg(0) // score
          .arg(stash.to_string()) // member
          .query_async(&mut conn as &mut Connection)
          .await
          .map_err(CacheError::RedisCMDError)?;

        // Cache statistical boards
        let _: () = redis::cmd("ZADD")
          .arg(Key::BoardAtEra(0, BOARD_JUDGEMENTS_VALIDATORS.to_string()))
          .arg(
            validator_data
              .get("judgements")
              .unwrap_or(&"0".to_string())
              .parse::<u32>()
              .ok()
              .unwrap_or_default(),
          ) // score
          .arg(stash.to_string()) // member
          .query_async(&mut conn as &mut Connection)
          .await
          .map_err(CacheError::RedisCMDError)?;

        let _: () = redis::cmd("ZADD")
          .arg(Key::BoardAtEra(
            0,
            BOARD_SUB_ACCOUNTS_VALIDATORS.to_string(),
          ))
          .arg(
            validator_data
              .get("sub_accounts")
              .unwrap_or(&"0".to_string())
              .parse::<u32>()
              .ok()
              .unwrap_or_default(),
          ) // score
          .arg(stash.to_string()) // member
          .query_async(&mut conn as &mut Connection)
          .await
          .map_err(CacheError::RedisCMDError)?;

        debug!("Successfully synced validator with stash {}", stash);
        i += 1;
      }
    }

    let _: () = redis::cmd("HSET")
      .arg(Key::Info)
      .arg(&[("validators", i.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    info!(
      "Successfully synced {} validators in era {}",
      i, active_era.index
    );

    Ok(())
  }

  /// Sync all nominators currently available
  async fn nominators(&self) -> Result<(), SyncError> {
    let mut conn = self
      .cache_pool
      .get()
      .await
      .map_err(CacheError::RedisPoolError)?;
    let client = self.node_client.clone();

    info!("Starting nominators sync");
    let mut nominators = client.nominators_iter(None).await?;
    let mut i = 0;
    while let Some((key, nominations)) = nominators.next().await? {
      let stash = get_account_id_from_storage_key(key);
      if let Some(controller) = client.bonded(stash.clone(), None).await? {
        let nominator_stake = self.get_controller_stake(&controller).await?;
        for validator_stash in nominations.targets.iter() {
          let exists: bool = redis::cmd("EXISTS")
            .arg(Key::Validator(validator_stash.clone()))
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;

          if !exists {
            warn!(
              "Skipping validator with stash {} -> no longer available",
              validator_stash
            );
            continue;
          }
          let _: () = redis::cmd("HINCRBY")
            .arg(Key::Validator(validator_stash.clone()))
            .arg("nominators")
            .arg(1)
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;

          // Since the range of values supported by HINCRBY is limited to 64 bit signed integers.
          // Store value as string and make calculation here
          let res: Option<String> = redis::cmd("HGET")
            .arg(Key::Validator(validator_stash.clone()))
            .arg("nominators_stake")
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
          let mut value = match res {
            Some(value) => value.parse::<u128>().unwrap_or_default(),
            None => 0,
          };
          value += nominator_stake;
          let _: () = redis::cmd("HSET")
            .arg(Key::Validator(validator_stash.clone()))
            .arg("nominators_stake")
            .arg(value.to_string())
            .query_async(&mut conn as &mut Connection)
            .await
            .map_err(CacheError::RedisCMDError)?;
        }
      }
      i += 1;
      debug!("Successfully synced nominator with stash {}", stash);
    }
    let _: () = redis::cmd("HSET")
      .arg(Key::Info)
      .arg(&[("nominators", i.to_string())])
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;
    info!("Successfully synced {} nominators", i);
    Ok(())
  }

  #[async_recursion]
  async fn get_identity(
    &self,
    stash: &AccountId32,
    sub_account_name: Option<String>,
  ) -> Result<BTreeMap<String, String>, SyncError> {
    let client = self.node_client.clone();
    let mut identity_data: BTreeMap<String, String> = BTreeMap::new();
    let re = Regex::new(r"[^\x20-\x7E]").unwrap();
    match client.identity_of(stash.clone(), None).await? {
      Some(registration) => {
        let mut parent =
          String::from_utf8(registration.info.display.encode()).unwrap_or("-".to_string());
        parent = re.replace_all(&parent, "").trim().to_string();
        // Name
        let name = if let Some(n) = sub_account_name {
          format!("{}/{}", parent, n)
        } else {
          parent
        };
        identity_data.insert("name".to_string(), name);
        // Judgements: [(0, Judgement::Reasonable)]
        let judgements = registration
          .judgements
          .into_iter()
          .fold(0, |acc, x| match x.1 {
            Judgement::Reasonable => acc + 1,
            Judgement::KnownGood => acc + 1,
            _ => acc,
          });
        identity_data.insert("judgements".to_string(), judgements.to_string());
        // Identity Sub-Accounts
        let (_, subs) = client.subs_of(stash.clone(), None).await?;
        identity_data.insert("sub_accounts".to_string(), subs.len().to_string());
      }
      None => {
        if let Some((parent_account, data)) = client.super_of(stash.clone(), None).await? {
          let mut sub_account_name = String::from_utf8(data.encode()).unwrap();
          sub_account_name = re.replace_all(&sub_account_name, "").trim().to_string();
          return self
            .get_identity(&parent_account, Some(sub_account_name))
            .await;
        } else {
          identity_data.insert("name".to_string(), "".to_string());
          identity_data.insert("judgements".to_string(), "0".to_string());
          identity_data.insert("sub_accounts".to_string(), "0".to_string());
        }
      }
    };
    Ok(identity_data)
  }

  async fn get_controller_stake(&self, controller: &AccountId32) -> Result<u128, SyncError> {
    let client = self.node_client.clone();
    let amount = if let Some(ledger) = client.ledger(controller.clone(), None).await? {
      ledger.active
    } else {
      0
    };
    Ok(amount)
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

  /// Calculate average reward points for all eras available
  async fn calculate_avg_reward_points(
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

    let avg = mean(&v);

    Ok(avg)
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
        warn!("No Validators available in the active era");
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
        .arg(Key::BoardAtEra(
          active_era.index,
          BOARD_ACTIVE_VALIDATORS.to_string(),
        ))
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
    let mut era_data: BTreeMap<String, String> = BTreeMap::new();
    let total = era_reward_points.total;
    era_data.insert("total_reward_points".to_string(), total.to_string());
    let min = min(&reward_points);
    era_data.insert("min_reward_points".to_string(), min.to_string());
    let max = max(&reward_points);
    era_data.insert("max_reward_points".to_string(), max.to_string());
    let avg = mean(&reward_points);
    era_data.insert("avg_reward_points".to_string(), avg.to_string());
    let median = median(&mut reward_points);
    era_data.insert("median_reward_points".to_string(), median.to_string());

    let _: () = redis::cmd("HSET")
      .arg(Key::Era(era_index))
      .arg(era_data)
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    // Cache statistical boards
    // TODO: delete old eras
    let _: () = redis::cmd("ZADD")
      .arg(Key::BoardAtEra(0, BOARD_TOTAL_POINTS_ERAS.to_string()))
      .arg(total) // score
      .arg(era_index) // member
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    let _: () = redis::cmd("ZADD")
      .arg(Key::BoardAtEra(0, BOARD_MAX_POINTS_ERAS.to_string()))
      .arg(max) // score
      .arg(era_index) // member
      .query_async(&mut conn as &mut Connection)
      .await
      .map_err(CacheError::RedisCMDError)?;

    let _: () = redis::cmd("ZADD")
      .arg(Key::BoardAtEra(0, BOARD_MIN_POINTS_ERAS.to_string()))
      .arg(min) // score
      .arg(era_index) // member
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

pub fn spawn_and_restart_era_payout_subscription_on_error() {
  task::spawn(async {
    loop {
      let sync: Sync = Sync::new().await;
      if let Err(e) = sync.subscribe_era_payout_events().await {
        error!("{}", e);
        thread::sleep(time::Duration::from_millis(500));
      };
    }
  });
}

#[allow(dead_code)]
pub fn spawn_and_restart_new_session_subscription_on_error() {
  task::spawn(async {
    loop {
      let sync: Sync = Sync::new().await;
      if let Err(e) = sync.subscribe_new_session_events().await {
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
