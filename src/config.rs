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

// Load environment variables into a Config struct
//
// Envy is a library for deserializing environment variables into
// typesafe structs
//
// Dotenv loads environment variables from a .env file, if available,
// and mashes those with the actual environment variables provided by
// the operative system.
//
// Set Config struct into a CONFIG lazy_static to avoid multiple processing.
//
use dotenv;
use lazy_static::lazy_static;
use log::info;
use serde::Deserialize;
use std::env;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
  pub turboflakes_host: String,
  pub turboflakes_port: u16,
  pub rust_backtrace: u8,
  pub rust_log: String,
  pub substrate_ws_url: String,
  pub redis_hostname: String,
  pub redis_password: String,
  pub redis_database: u8,
}

// Set Config struct into a CONFIG lazy_static to avoid multiple processing
lazy_static! {
  pub static ref CONFIG: Config = get_config();
}

/// Inject dotenv and env vars into the Config struct
fn get_config() -> Config {
  let config_filename = env::var("TURBOFLAKES_CONFIG_FILENAME").unwrap_or(".env".to_string());
  dotenv::from_filename(&config_filename).ok();

  env_logger::try_init().unwrap_or_default();
  
  info!("loading configuration from {}", &config_filename);

  match envy::from_env::<Config>() {
    Ok(config) => config,
    Err(error) => panic!("configuration error: {:#?}", error),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_gets_a_config() {
    let config = get_config();
    assert_ne!(config.rust_log, "".to_string());
  }

  #[test]
  fn it_gets_a_config_from_the_lazy_static() {
    let config = &CONFIG;
    assert_ne!(config.rust_log, "".to_string());
  }
}
