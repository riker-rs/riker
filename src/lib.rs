#![crate_name = "riker"]
#![feature(
        async_await,
        await_macro,
        futures_api,
        arbitrary_self_types
)]

mod validate;

pub mod actor;
pub mod kernel;
pub mod model;
pub mod protocol;
pub mod system;

use std::env;
use std::error::Error;
use std::fmt;

use futures::Future;
use futures::future::RemoteHandle;
use config::{Config, File};

pub trait ExecutionContext {
    fn execute<F>(&self, f: F) -> RemoteHandle<ExecResult<F::Output>>
        where F: Future + Send + 'static,
                <F as Future>::Output: std::marker::Send;
}

pub type ExecResult<T> = Result<T, ExecError>;

pub struct ExecError;

impl Error for ExecError {
    fn description(&self) -> &str {
        "Panic occurred during execution of the task."
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Panic occurred during execution of the task.")
    }
}

impl fmt::Debug for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
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
    // riker.toml contains settings for anything related to the actor framework and its modules
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
    pub use crate::model::Model;
    pub use crate::protocol::{Message, ActorMsg, ChannelMsg, Identify, SystemEnvelope, SystemMsg, SystemEvent, IOMsg, ESMsg, CQMsg};
    pub use crate::ExecutionContext;
    pub use crate::actor::*;
    pub use crate::system::{ActorSystem, Evt,Timer};
    pub use crate::load_config;
}