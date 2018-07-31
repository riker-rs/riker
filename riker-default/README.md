# Riker Default Model

## Overview

This is default model that provides default modules for all core services. In many cases this model works well even for production environments.

If you're looking for the main Riker repository please see [Riker](https://github.com/riker-rs/riker).

The official Riker documentation explains how to use the features that these modules provide. You can find the documentation [here](http://riker.rs).

[![Build Status](https://travis-ci.org/riker-rs/riker-default.svg?branch=master)](https://travis-ci.org/riker-rs/riker-default)

To use the default model in your actor system:

```rust
extern crate riker;
extern crate riker_default;
 
use riker::actors::*;
use riker_default::DefaultModel;
 
// Get a default model with String as the message type
let model: DefaultModel<String> = DefaultModel::new();
let sys = ActorSystem::new(&model).unwrap();
```

## Modules

- Dispatcher: [`riker-dispatcher`](riker-dispatcher)
- Dead Letters: [`riker-deadletters`](riker-deadletters)
- Timer: [`riker-timer`](riker-timer)
- Event Store: [`riker-mapvec`](riker-mapvec)
- Log: [`riker-log`](riker-log)

Default modules are maintained as part of the same git repository but each is a separate crate. This allows custom models to use individual modules without needing to pull in other crates.


