# turboflakes-backend 

Turboflakes is a service that makes it fast and easy to interact with Substrate-based blockchain nodes

![latest release](https://github.com/turboflakes/turboflakes-backend/actions/workflows/create_release.yml/badge.svg)

## Run

```bash
#!/bin/bash
$ cargo run
```

## Available endpoints

Index endpoint

```bash
#!/bin/bash
$ curl http://0.0.0.0:5000

{
    "name": "turboflakes-backend",
    "version": "0.3.0",
    "api_path": "/api/v1",
    "substrate_node_url": "wss://westend-rpc.polkadot.io"
}
```

Health endpoint

```bash
#!/bin/bash
$ curl http://0.0.0.0:5000/health

{
  "status": "ok",
}
```

Era endpoints

```bash
#!/bin/bash
curl http://0.0.0.0:5000/api/v1/era/{era_index}

{
    "era_index": {era_index},
    "total_reward": 568509436507540,
    "total_stake": 5586408452650880117,
    "total_reward_points": 70540,
    "min_reward_points": 20,
    "max_reward_points": 260,
    "mean_reward_points": 0,
    "median_reward_points": 80
}
```

Validator endpoints

```bash
#!/bin/bash
curl http://localhost:5000/api/v1/validator/{stash}

{
    "stash": "{stash}",
    "controller": "controller",
    "name": "TURBOFLAKES",
    "nominators": 3,
    "inclusion_rate": 0.14,
    "mean_reward_points": 7480,
    "commission": 1,
    "blocked": false,
    "active": true,
    "reward_staked": true
}
```

```bash
#!/bin/bash
curl http://localhost:5000/api/v1/validator/{stash}/eras

{
    "stash": "{stash}",
    "eras": [
        {
          "era_index": {era_index}
          "own_stake": 1340256205460046,
          "total_stake": 2436430707131921,
          "others_stake": 1096174501671875,
          "reward_points": 7480,
          "commission": 1,
          "blocked": false,
          "active": true
        }
        ...
    ]
}
```

## Development

Recompile the code on changes and run the binary

```bash
#!/bin/bash
# default .env
$ cargo watch -x 'run --bin turboflakes-backend'
# or specific .env.kusama configuration file
$ TURBOFLAKES_CONFIG_FILENAME=.env.kusama cargo watch -x 'run --bin turboflakes-backend'
```

### Inspiration

Projects that had influence in turboflakes-backend design and helped to solve technical barriers.

- [Substrate - Substrate is a next-generation framework for blockchain innovation ????.](https://github.com/paritytech/substrate)
- [Polkadot - Implementation of a https://polkadot.network node in Rust based on the Substrate framework.](https://github.com/paritytech/polkadot)
- [subxt - A library to submit extrinsics to a substrate node via RPC.](https://github.com/paritytech/substrate-subxt)
- [Actix - A powerful, pragmatic, and extremely fast web framework for Rust](https://actix.rs/)
- [Actix - Examples](https://github.com/actix/examples)
- [Actix - An Actix 2.0 REST server using the Rust language.](https://github.com/ddimaria/rust-actix-example)
- [Redis library for Rust](https://github.com/mitsuhiko/redis-rs)

Books

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)

Articles

- [Using Redis in a Rust web service](https://blog.logrocket.com/using-redis-in-a-rust-web-service/)
