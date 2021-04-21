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

use crate::config::{Config, CONFIG};
use crate::errors::{CacheError};

use actix_web::web;
use mobc::{Connection, Pool};
use mobc_redis::RedisConnectionManager;
use std::time::Duration;

const CACHE_POOL_MAX_OPEN: u64 = 20;
const CACHE_POOL_MAX_IDLE: u64 = 8;
const CACHE_POOL_TIMEOUT_SECONDS: u64 = 1;
const CACHE_POOL_EXPIRE_SECONDS: u64 = 60;

pub type RedisPool = Pool<RedisConnectionManager>;
pub type RedisConn = Connection<RedisConnectionManager>;

fn get_redis_url(config: Config) -> String {
  format!(
    "redis://:{}@{}/{}",
    config.redis_password, config.redis_hostname, config.redis_database
  )
  .to_string()
}

pub fn create_pool(config: Config) -> Result<RedisPool, CacheError> {
  let redis_url = get_redis_url(config);
  let client = redis::Client::open(redis_url).map_err(CacheError::RedisClientError)?;
  let manager = RedisConnectionManager::new(client);
  Ok(
    Pool::builder()
      .get_timeout(Some(Duration::from_secs(CACHE_POOL_TIMEOUT_SECONDS)))
      .max_open(CACHE_POOL_MAX_OPEN)
      .max_idle(CACHE_POOL_MAX_IDLE)
      .max_lifetime(Some(Duration::from_secs(CACHE_POOL_EXPIRE_SECONDS)))
      .build(manager),
  )
}

pub fn add_pool(cfg: &mut web::ServiceConfig) {
  let pool = create_pool(CONFIG.clone()).expect("failed to create Redis pool");
  cfg.data(pool);
}

pub async fn get_conn(pool: &RedisPool) -> Result<RedisConn, CacheError> {
  pool
    .get()
    .await
    .map_err(CacheError::RedisPoolError)
}
