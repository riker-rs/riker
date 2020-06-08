pub(crate) mod actor;
pub(crate) mod actor_cell;
pub(crate) mod actor_ref;
pub(crate) mod channel;
pub(crate) mod macros;
pub(crate) mod props;
pub(crate) mod selection;
pub(crate) mod uri;

use std::{error::Error, fmt};

use crate::validate::InvalidName;

// Public riker::actor API (plus the pub data types in this file)
pub use self::{
    actor::{Actor, BoxActor, Receive, Strategy},
    actor_cell::Context,
    actor_ref::{
        ActorRef, ActorRefFactory, ActorReference, BasicActorRef, BoxedTell, Sender, Tell,
        TmpActorRefFactory,
    },
    channel::{
        channel, All, Channel, ChannelMsg, ChannelRef, DLChannelMsg, DeadLetter, EventsChannel,
        Publish, Subscribe, SysTopic, Topic, Unsubscribe, UnsubscribeAll,
    },
    macros::actor,
    props::{ActorArgs, ActorFactory, ActorFactoryArgs, ActorProducer, BoxActorProd, Props},
    selection::{ActorSelection, ActorSelectionFactory},
    uri::{ActorId, ActorPath, ActorUri, AtomicActorId},
};

#[allow(unused)]
pub type MsgResult<T> = Result<(), MsgError<T>>;

/// Internal message error when a message can't be added to an actor's mailbox
#[doc(hidden)]
#[derive(Clone)]
pub struct MsgError<T> {
    pub msg: T,
}

impl<T> MsgError<T> {
    pub fn new(msg: T) -> Self {
        MsgError { msg }
    }
}

impl<T> Error for MsgError<T> {
    fn description(&self) -> &str {
        "The actor does not exist. It may have been terminated"
    }
}

impl<T> fmt::Display for MsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl<T> fmt::Debug for MsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// Error type when an `try_tell` fails on `Option<ActorRef<Msg>>`
pub struct TryMsgError<T> {
    pub msg: T,
}

impl<T> TryMsgError<T> {
    pub fn new(msg: T) -> Self {
        TryMsgError { msg }
    }
}

impl<T> Error for TryMsgError<T> {
    fn description(&self) -> &str {
        "Option<ActorRef> is None"
    }
}

impl<T> fmt::Display for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl<T> fmt::Debug for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// Error type when an actor fails to start during `actor_of`.
pub enum CreateError {
    Panicked,
    System,
    InvalidName(String),
    AlreadyExists(ActorPath),
}

impl Error for CreateError {
    fn description(&self) -> &str {
        match *self {
            CreateError::Panicked => "Failed to create actor. Cause: Actor panicked while starting",
            CreateError::System => "Failed to create actor. Cause: System failure",
            CreateError::InvalidName(_) => "Failed to create actor. Cause: Invalid actor name",
            CreateError::AlreadyExists(_) => {
                "Failed to create actor. Cause: An actor at the same path already exists"
            }
        }
    }
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreateError::Panicked => f.write_str(&self.to_string()),
            CreateError::System => f.write_str(&self.to_string()),
            CreateError::InvalidName(ref name) => {
                f.write_str(&format!("{} ({})", self.to_string(), name))
            }
            CreateError::AlreadyExists(ref path) => {
                f.write_str(&format!("{} ({})", self.to_string(), path))
            }
        }
    }
}

impl fmt::Debug for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl From<InvalidName> for CreateError {
    fn from(err: InvalidName) -> CreateError {
        CreateError::InvalidName(err.name)
    }
}

/// Error type when an actor fails to restart.
pub struct RestartError;

impl Error for RestartError {
    fn description(&self) -> &str {
        "Failed to restart actor. Cause: Actor panicked while starting"
    }
}

impl fmt::Display for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl fmt::Debug for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}
