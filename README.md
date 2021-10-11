# Tezedge Actor System

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![pre-commit](https://github.com/tezedge/tezedge-actor-system/actions/workflows/pre-commit.yml/badge.svg)](https://github.com/tezedge/tezedge-actor-system/actions/workflows/pre-commit.yml)

## Overview

TezEdge Actor System is a fork of riker. It is a framework for building safe, modern, concurrent and resilient systems using the Rust language. It focuses on simplicity and safety. TezEdge Actor System aims to make working with state and behavior in concurrent systems as easy and scalable as possible. The Actor Model has been chosen to realize this because of the familiar and inherent simplicity it provides while also providing strong guarantees that are easy to reason about.

TezEdge Actor System provides:

- An Actor based execution runtime
- Concurrency built on `tokio`
- Publish/Subscribe messaging via actor channels
- Message scheduling
- Out-of-the-box, configurable, non-blocking logging

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

Official crates that provide additional functionality:

- [riker-testkit](https://github.com/riker-rs/riker-testkit): Tools to make testing easier

## Why TezEdge Actor System

TezEdge Actor System is a step to improve safety of the TezEdge node.

Rust empowers developers with control over memory management, requiring no garbage collection and runtime overhead, while also providing modern semantics and expressive syntax such as the trait system. The result is a language that can solve problems equally for Web and IoT.

TezEdge Actor System adds to this by providing a familiar actor model API which in turn makes concurrent, resilient systems programming easy.

## Rust Version

TezEdge Actor System is currently built using the Rust version `nightly-2021-08-04`, like other TezEdge projects do.
