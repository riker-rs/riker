#![crate_name = "riker"]
#![feature(
        async_await,
        await_macro,
        futures_api,
        arbitrary_self_types
)]

#![allow(warnings)] // todo

#[allow(unused_imports)]
extern crate log;

mod validate;

pub mod actor;
pub mod kernel;
pub mod system;

use std::env;
use std::fmt::Debug;
use std::fmt;
use std::any::Any;


use config::{Config, File};

use crate::actor::BasicActorRef;

pub fn load_config() -> Config {
    let mut cfg = Config::new();

    cfg.set_default("debug", true).unwrap();
    cfg.set_default("log.level", "debug").unwrap();
    cfg.set_default("log.log_format", "{date} {time} {level} [{module}] {body}").unwrap();
    cfg.set_default("log.date_format", "%Y-%m-%d").unwrap();
    cfg.set_default("log.time_format", "%H:%M:%S%:z").unwrap();
    cfg.set_default("mailbox.msg_process_limit", 1000).unwrap();
    cfg.set_default("dispatcher.pool_size", 4).unwrap();
    cfg.set_default("scheduler.frequency_millis", 50).unwrap();

    // load the system config
    // riker.toml contains settings for anything related to the actor framework and its modules
    let path = env::var("RIKER_CONF").unwrap_or("config/riker.toml".into());
    cfg.merge(File::with_name(&format!("{}", path)).required(false)).unwrap();

    // load the user application config
    // app.toml or app.yaml contains settings specific to the user application
    let path = env::var("APP_CONF").unwrap_or("config/app".into());
    cfg.merge(File::with_name(&format!("{}", path)).required(false)).unwrap();
    cfg
}

/// Wraps message and sender
#[derive(Debug, Clone)]
pub struct Envelope<T: Message> {
    pub sender: Option<BasicActorRef>,
    pub msg: T,
}

unsafe impl<T: Message> Send for Envelope<T> {}

pub trait Message: Debug + Clone + Send + 'static {}
impl<T: Debug + Clone + Send + 'static> Message for T {}


pub struct AnyMessage {
    pub msg: Box<Any + Send>,
}

impl Clone for AnyMessage {
    fn clone(&self) -> Self {
        panic!("Can't clone a message of type `AnyMessage`");
    }
}

// impl fmt::Display for MsgError<T> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         f.write_str(self.description())
//     }
// }

impl Debug for AnyMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AnyMessage")
    }
}

pub mod actors {
    pub use crate::{Message, AnyMessage};
    pub use crate::actor::*;
    pub use crate::system::{ActorSystem, SystemMsg, SystemEvent};
}