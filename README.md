# TezEdge Actor System

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Drone status](http://ci.tezedge.com/api/badges/tezedge/tezedge-actor-system/status.svg)](http://ci.tezedge.com/tezedge/tezedge-actor-system)
[![Build and run tests](https://github.com/tezedge/tezedge-actor-system/actions/workflows/build-and-test.yml/badge.svg)](https://github.com/tezedge/tezedge-actor-system/actions/workflows/build-and-test.yml)
[![Audit](https://github.com/tezedge/tezedge-actor-system/actions/workflows/audit.yml/badge.svg)](https://github.com/tezedge/tezedge-actor-system/actions/workflows/audit.yml)
[![pre-commit](https://github.com/tezedge/tezedge-actor-system/actions/workflows/pre-commit.yml/badge.svg)](https://github.com/tezedge/tezedge-actor-system/actions/workflows/pre-commit.yml)

## Overview

The TezEdge Actor System is a fork of Riker. It is a framework for building safe, modern, concurrent and resilient systems using the Rust language. It focuses on safety and simplicity. TezEdge Actor System aims to make work with states and behavior in concurrent systems as easy and scalable as possible. The Actor Model has been chosen because of its familiarity and inherent simplicity while also providing strong guarantees that are easy to reason with.

The TezEdge Actor System provides:

- An Actor-based execution runtime
- Concurrency built on `tokio`
- Publish/Subscribe messaging via actor channels
- Message scheduling
- Out-of-the-box, configurable, non-blocking logging

## Why did we create the TezEdge Actor System?

TezEdge Actor System is a step towards improving the safety of the TezEdge node.

Rust empowers developers with control over memory management, requiring no garbage collection and runtime overhead, while also providing modern semantics and expressive syntax such as the trait system. TezEdge Actor System adds to this by providing a familiar actor model API which in turn makes programming concurrent, resilient systems easy.

In the TezEdge node, safety is of utmost importance. The original actor system (Riker) was a simple solution, but we knew that there was room for improvement in regards to safety. Riker itself has some unsafe code which might cause bugs or even a breach of security and it has a lot of unsafe dependencies. This fork rewrites the unsafe parts of Riker and its dependencies. Surprisingly, we succeeded in achieving that without sacrificing performance. Additionally, the fork removes unneeded (for the TezEdge node) functionalities in order to reduce the node's attack surface.

In the fork we are using `tokio` crate instead of `futures` crate to run async tasks. It is the only part that containing unsafe code. We are of the opinion that `tokio` is safe, because it is used almost everywhere in crypto applications written in Rust, and therefore it is well tested in production.

## Example

`Cargo.toml`:

```toml
[dependencies]
tezedge-actor-system = { git = "https://github.com/tezedge/tezedge-actor-system.git", tag = "v0.4.2-cleanup-unsafe-8" }
```

`main.rs`:

```rust
use std::time::Duration;
use tezedge_actor_system::actors::*;

#[derive(Default)]
struct MyActor;

// implement the Actor trait
impl Actor for MyActor {
    type Msg = String;

    fn recv(&mut self,
                _ctx: &Context<String>,
                msg: String,
                _sender: Sender) {

        println!("Received: {}", msg);
    }
}

// start the system and create an actor
#[tokio::main]
async fn main() {
    let backend = tokio::runtime::Handle::current().into();
    let sys = ActorSystem::new(backend).unwrap();

    let my_actor = sys.actor_of::<MyActor>("my-actor").unwrap();

    my_actor.tell("Hello my actor!".to_string(), None);

    tokio::time::sleep(Duration::from_millis(500)).await;
}
```

## Associated Projects

Official crates that provide additional functionalities:

- [riker-testkit](https://github.com/riker-rs/riker-testkit): Tools to make testing easier

## Rust Version

The TezEdge Actor System is currently built using the Rust version `nightly-2021-08-04`, same as the other projects under the TezEdge node.

## pre-commit

Before you commit your code, [pre-commit](https://pre-commit.com/) integrates as a git hook to automatically check your code.
Please don't skip git hooks (even if you do the travis TravisCI build, it will still fail).

There are two different approaches you can use to run pre-commit

#### direct approach

```bash
pre-commit run -a
```

#### with yarn or npm

```bash
yarn
yarn lint
```

```bash
npm run install
npn run lint
```

## Cargo Geiger Safety Report

```

Metric output format: x/y
    x = unsafe code used by the build
    y = total unsafe code found in the crate

Symbols:
    🔒  = No `unsafe` usage found, declares #![forbid(unsafe_code)]
    ❓  = No `unsafe` usage found, missing #![forbid(unsafe_code)]
    ☢️  = `unsafe` usage found

Functions  Expressions  Impls  Traits  Methods  Dependency

0/0        0/0          0/0    0/0     0/0      🔒  tezedge-actor-system 0.5.0
0/0        0/0          0/0    0/0     0/0      ❓  ├── slog 2.7.0
0/0        0/0          0/0    0/0     0/0      ❓  ├── tezedge-actor-system-macros 0.5.0
0/0        0/0          0/0    0/0     0/0      ❓  │   ├── proc-macro2 1.0.30
0/0        0/0          0/0    0/0     0/0      🔒  │   │   └── unicode-xid 0.2.2
0/0        0/0          0/0    0/0     0/0      ❓  │   ├── quote 1.0.10
0/0        0/0          0/0    0/0     0/0      ❓  │   │   └── proc-macro2 1.0.30
0/0        45/45        3/3    0/0     2/2      ☢️  │   └── syn 1.0.80
0/0        0/0          0/0    0/0     0/0      ❓  │       ├── proc-macro2 1.0.30
0/0        0/0          0/0    0/0     0/0      ❓  │       ├── quote 1.0.10
0/0        0/0          0/0    0/0     0/0      🔒  │       └── unicode-xid 0.2.2
20/25      1269/1804    82/102 1/1     59/69    ☢️  └── tokio 1.12.0
0/17       0/630        0/13   0/1     0/19     ❓      ├── bytes 1.1.0
0/20       12/319       0/0    0/0     2/30     ☢️      ├── libc 0.2.103
0/0        72/72        0/0    0/0     0/0      ☢️      ├── num_cpus 1.13.0
0/20       12/319       0/0    0/0     2/30     ☢️      │   └── libc 0.2.103
0/0        8/167        0/0    0/0     0/0      ☢️      ├── pin-project-lite 0.2.7
0/0        0/0          0/0    0/0     0/0      ❓      └── tokio-macros 1.5.0
0/0        0/0          0/0    0/0     0/0      ❓          ├── proc-macro2 1.0.30
0/0        0/0          0/0    0/0     0/0      ❓          ├── quote 1.0.10
0/0        45/45        3/3    0/0     2/2      ☢️          └── syn 1.0.80

20/62      1406/3037    85/118 1/2     63/120

```
