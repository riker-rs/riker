# Riker
NOTE: April 2019. We're making some major design changes to Riker. During this time PRs and issues aren't getting processed as fast as usual. We hope to get a new major release out soon!

[![Build Status](https://travis-ci.org/riker-rs/riker.svg?branch=master)](https://travis-ci.org/riker-rs/riker)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](https://meritbadge.herokuapp.com/riker)](https://crates.io/crates/riker)
[![Released API docs](https://docs.rs/riker/badge.svg)](https://docs.rs/riker)

## Overview

Riker is a framework for building modern, concurrent and resilient systems using the Rust language. Riker aims to make working with state and behavior in concurrent systems as easy and scalable as possible. The Actor Model has been chosen to realize this because of the familiar and inherent simplicity it provides while also providing strong guarantees that are easy to reason about. The Actor Model also provides a firm foundation for resilient systems through the use of the actor hierarchy and actor supervision.

Riker provides:

- An Actor based execution runtime
- Actor supervision to isolate and recover from failures
- A modular system
- A Dispatcher backed by `futures::execution::ThreadPool` by default
- Publish/Subscribe messaging via actor channels
- Message scheduling
- Out-of-the-box, configurable, non-blocking logging
- Persistent actors using Event Sourcing
- Command Query Responsiblily Separation (CQRS)
- Run futures alongside actors

[Website](http://riker.rs) | [API Docs](https://docs.rs/riker)

## Example

`Cargo.toml`:
```toml
[dependencies]
riker = "0.1.8"
riker-default = "0.1.8"
```

`main.rs`:
```rust
extern crate riker;
extern crate riker_default;
#[macro_use]
extern crate log;

use std::time::Duration;
use riker::actors::*;
use riker_default::DefaultModel;

struct MyActor;

// implement the Actor trait
impl Actor for MyActor {
    type Msg = String;

    fn receive(&mut self,
                _ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                _sender: Option<ActorRef<Self::Msg>>) {

        debug!("Received: {}", msg);
    }
}

// provide factory and props methods
impl MyActor {
    fn actor() -> BoxActor<String> {
        Box::new(MyActor)
    }

    fn props() -> BoxActorProd<String> {
        Props::new(Box::new(MyActor::actor))
    }
}

// start the system and create an actor
fn main() {
    let model: DefaultModel<String> = DefaultModel::new();
    let sys = ActorSystem::new(&model).unwrap();

    let props = MyActor::props();
    let my_actor = sys.actor_of(props, "my-actor").unwrap();

    my_actor.tell("Hello my actor!".to_string(), None);

    std::thread::sleep(Duration::from_millis(500));
}
```

## Modules

Riker's core functionality is provided by modules defined as part of a 'model'. Every application defines a `Model` to describe the modules to be used. Everything from database storage, to logging, to the underlying dispatcher that executes actors is a module.

A default `Model` [riker-default](riker-default) makes it easy to get started. You can also use this default model as part of a custom model.

## Associated Projects

Official crates that provide additional functionality:

- [riker-cqrs](https://github.com/riker-rs/riker-cqrs): Command Query Responsibility Separation support
- [riker-testkit](https://github.com/riker-rs/riker-testkit): Tools to make testing easier
- [riker-patterns](https://github.com/riker-rs/riker-patterns): Common actor patterns, including `transform!` and 'ask'

## Roadmap & Currently in Development

The next major theme on the project roadmap is clustering and location transparency:

- Remote actors
- Support for TCP and UDP
- Clustering (using vector clocks)
- Distributed data (CRDTs)

## Why Riker
We believe there is no greater need than now for a full-featured actor model implementation that scales to hundreds or thousands of microservices and that equally can run exceptionally well on resource limited hardware to drive drones, IoT and robotics. The Rust language makes this possible.

Rust empowers developers with control over memory management, requiring no garbage collection and runtime overhead, while also providing modern semantics and expressive syntax such as the trait system. The result is a language that can solve problems equally for Web and IoT.

Riker adds to this by providing a famililar actor model API which in turn makes concurrent, resilent systems programming easy.

## Rust Version
Riker is currently built using the **latest Rust Nightly**. Starting from between Riker `0.2` and `0.3` we expect to build against Rust Stable with support for specific minimum versions.

## Contributing

Riker is looking for contributors - join the project! You don't need to be an expert in actors, concurrent systems, or even Rust. Great ideas come from everyone.

There are multiple ways to contribute:

- Ask questions. Adding to the conversation is a great way to contribute. Find us on [Gitter](https://gitter.im/riker-rs/Lobby).
- Documentation. Our aim is to make concurrent, resilient systems programming available to everyone and that starts with great Documentation.
- Additions to Riker code base. Whether small or big, your Pull Request could make a difference.
- Patterns, data storage and other supporting crates. We are happy to link to and provide full credit to external projects that provide support for databases in Riker's event storage model or implementations of common actor patterns.


