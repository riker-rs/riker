mod io;
mod logger;
mod persist;
mod timer;
mod system;

use std::error::Error;
use std::fmt;

pub use self::system::ActorSystem;
pub use self::persist::{Evt, EsManager, EventStore, NoEventStore, query};
pub use self::logger::{LogEntry, LoggerProps, DeadLetterProps};
pub use self::io::{Io, IoType, Tcp, Udp, IoManagerProps, IoAddress, NoIo};
pub use self::timer::{Timer, TimerFactory, Job, OnceJob, RepeatJob};

pub enum SystemError {
    ModuleFailed(String),
    InvalidName(String),
}

impl Error for SystemError {
    fn description(&self) -> &str {
        match *self {
            SystemError::ModuleFailed(_) => "Failed to create actor system. Cause: Sub module failed to start",
            SystemError::InvalidName(_) => "Failed to create actor system. Cause: Invalid actor system name",
        }
    }
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SystemError::ModuleFailed(ref m) => f.write_str(&format!("{} ({})", self.description(), m)),
            SystemError::InvalidName(ref name) => f.write_str(&format!("{} ({})", self.description(), name)),
        }
    }
}

impl fmt::Debug for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}