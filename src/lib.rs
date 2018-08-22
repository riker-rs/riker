#![crate_name = "riker"]
#![feature(assoc_unix_epoch)]

extern crate bytes;
extern crate chrono;
extern crate config;
extern crate futures;
extern crate regex;
#[macro_use] 
extern crate log;
extern crate rand;
extern crate uuid;

mod validate;

pub mod actor;
pub mod futures_util;
pub mod kernel;
pub mod model;
pub mod protocol;
pub mod system;

use std::env;

use futures::Future;
use config::{Config, File};

use futures_util::DispatchHandle;

pub trait ExecutionContext {
    fn execute<F>(&self, f: F) -> DispatchHandle<F::Item, F::Error>
        where F: Future + Send + 'static,
                F::Item: Send + 'static,
                F::Error: Send + 'static;
}

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
    // am.toml contains settings for anything related to the actor framework and its modules
    let path = env::var("RIKER_CONF").unwrap_or("config/riker.toml".into());
    cfg.merge(File::with_name(&format!("{}", path)).required(false)).unwrap();

    // load the user application config
    // app.toml or app.yaml contains settings specific to the user application
    let path = env::var("APP_CONF").unwrap_or("config/app".into());
    cfg.merge(File::with_name(&format!("{}", path)).required(false)).unwrap();
    
    cfg
}

// Pub exports
pub mod actors {
    pub use model::Model;
    pub use protocol::{Message, ActorMsg, ChannelMsg, Identify, SystemEnvelope, SystemMsg, SystemEvent, IOMsg, ESMsg, CQMsg};
    pub use ExecutionContext;
    pub use actor::*;
    pub use system::{ActorSystem, Evt,Timer};
    pub use load_config;
    // pub use errors::Error as AMError;
}