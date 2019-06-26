pub(crate) mod logger;
pub(crate) mod system;
pub(crate) mod timer;

use std::{
    fmt,
    error::Error
};

use crate::actor::BasicActorRef;

// Public riker::system API (plus the pub data types in this file)
pub use self::{
    system::{ActorSystem, SystemBuilder, Run},
    timer::{Timer, BasicTimer},
    logger::LogEntry
};

#[derive(Clone, Debug)]
pub enum SystemMsg {
    ActorInit,
    Command(SystemCmd),
    Event(SystemEvent),
    Failed(BasicActorRef),
}

unsafe impl Send for SystemMsg {}

#[derive(Clone, Debug)]
pub enum SystemCmd {
    Stop,
    Restart,
}

impl Into<SystemMsg> for SystemCmd {
    fn into(self) -> SystemMsg {
        SystemMsg::Command(self)
    }
}

#[derive(Clone, Debug)]
pub enum SystemEvent {
    /// An actor was terminated
    ActorCreated(ActorCreated),

    /// An actor was restarted
    ActorRestarted(ActorRestarted),

    /// An actor was started
    ActorTerminated(ActorTerminated),
}

impl Into<SystemMsg> for SystemEvent {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(self)
    }
}

#[derive(Clone, Debug)]
pub struct ActorCreated {
    pub actor: BasicActorRef,
}

#[derive(Clone, Debug)]
pub struct ActorRestarted {
    pub actor: BasicActorRef,
}

#[derive(Clone, Debug)]
pub struct ActorTerminated {
    pub actor: BasicActorRef,
}

impl Into<SystemEvent> for ActorCreated {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorCreated(self)
    }
}

impl Into<SystemEvent> for ActorRestarted {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorRestarted(self)
    }
}

impl Into<SystemEvent> for ActorTerminated {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorTerminated(self)
    }
}

impl Into<SystemMsg> for ActorCreated {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorCreated(self))
    }
}

impl Into<SystemMsg> for ActorRestarted {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorRestarted(self))
    }
}

impl Into<SystemMsg> for ActorTerminated {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorTerminated(self))
    }
}

#[derive(Clone, Debug)]
pub enum SystemEventType {
    ActorTerminated,
    ActorRestarted,
    ActorCreated,
}

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
