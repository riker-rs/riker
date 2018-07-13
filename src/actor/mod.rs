mod actor;
mod actor_cell;
mod actor_ref;
mod channel;
mod props;
mod selection;
mod uri;

use std::error::Error;
use std::fmt;

use protocol::{Message, ActorMsg, SystemMsg};
// use self::actor_ref::ActorRef;


// pub mod public {
//     pub use actor::actor::{Actor, BoxActor, Strategy};
//     pub use actor::actor_cell::{ActorCell, CellPublic, Context, PersistenceConf};
//     pub use actor::actor_ref::{ActorRef, ActorRefFactory, TmpActorRefFactory};
//     pub use actor::uri::{ActorUri, ActorId};
//     pub use actor::selection::{ActorSelection, ActorSelectionFactory};
//     pub use actor::props::{Props, BoxActorProd, ActorProducer, ActorArgs};
//     pub use actor::channel::{Channel, SystemChannel, Topic, All, SysTopic, SysChannels, dead_letter};
// }

pub use self::actor::{Actor, BoxActor, Strategy};
pub use self::actor_cell::{ActorCell, CellPublic, Context, PersistenceConf};
pub use self::actor_ref::{ActorRef, ActorRefFactory, TmpActorRefFactory};
pub use self::uri::{ActorUri, ActorId};
pub use self::selection::{ActorSelection, ActorSelectionFactory};
pub use actor::props::{Props, BoxActorProd, ActorProducer, ActorArgs};
pub use self::channel::{Channel, SystemChannel, Topic, All, SysTopic, SysChannels, dead_letter};

pub use self::actor_cell::CellInternal;

pub trait Tell {
    type Msg: Message;
    
    /// Implement to provide message routing to actors, e.g. `ActorRef` and `ActorSelection`
    #[allow(unused)]
    fn tell<T>(&self,
                msg: T,
                sender: Option<ActorRef<Self::Msg>>)
        where T: Into<ActorMsg<Self::Msg>>;
            
}

/// Implement to provide possible message routing to actors, e.g. `Option<ActorRef>`
pub trait TryTell {
    type Msg: Message;
    
    #[allow(unused)]
    fn try_tell<T>(&self,
                msg: T,
                sender: Option<ActorRef<Self::Msg>>) -> Result<(), TryMsgError<T>>
        where T: Into<ActorMsg<Self::Msg>>;
            
}

/// Implement to provide system message routing to actors, e.g. `ActorRef` and `ActorSelection`
pub trait SysTell {
    type Msg: Message;

    fn sys_tell(&self,
                sys_msg: SystemMsg<Self::Msg>,
                sender: Option<ActorRef<Self::Msg>>);
}

#[allow(unused)]
pub type MsgResult<T> = Result<(), MsgError<T>>;

/// Internal message error when a message can't be added to an actor's mailbox
#[doc(hidden)]
pub struct MsgError<T> {
    pub msg: T,
}

impl<T> MsgError<T> {
    pub fn new(msg: T) -> Self {
        MsgError {
            msg
        }
    }
}

impl<T> Error for MsgError<T> {
    fn description(&self) -> &str {
        "The actor does not exist. It may have been terminated"
    }
}

impl<T> fmt::Display for MsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl<T> fmt::Debug for MsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// Error type when an `try_tell` fails on `Option<ActorRef<Msg>>`
pub struct TryMsgError<T> {
    pub msg: T,
} 

impl<T> TryMsgError<T> {
    pub fn new(msg: T) -> Self {
        TryMsgError {
            msg
        }
    }
}

impl<T> Error for TryMsgError<T> {
    fn description(&self) -> &str {
        "Option<ActorRef> is None"
    }
}

impl<T> fmt::Display for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl<T> fmt::Debug for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// Error type when an actor fails to start during `actor_of`.
pub enum CreateError {
    Panicked,
    InvalidName(String),
    AlreadyExists(String),
}

impl Error for CreateError {
    fn description(&self) -> &str {
        match *self {
            CreateError::Panicked => "Failed to create actor. Cause: Actor panicked while starting",
            CreateError::InvalidName(_) => "Failed to create actor. Cause: Invalid actor name",
            CreateError::AlreadyExists(_) => "Failed to create actor. Cause: An actor at the same path already exists"
        }
    }
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreateError::Panicked => f.write_str(self.description()),
            CreateError::InvalidName(ref name) => f.write_str(&format!("{} ({})", self.description(), name)),
            CreateError::AlreadyExists(ref path) => f.write_str(&format!("{} ({})", self.description(), path))
        }
    }
}

impl fmt::Debug for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
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
        f.write_str(self.description())
    }
}

impl fmt::Debug for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}
