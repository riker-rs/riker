#![allow(unused_variables)]
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

use crate::{system::SystemMsg, Message};

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

pub trait Actor: Send + 'static {
    type Msg: Message;

    /// Invoked when an actor is being started by the system.
    ///
    /// Any initialization inherent to the actor's role should be
    /// performed here.
    ///
    /// Panics in `pre_start` do not invoke the
    /// supervision strategy and the actor will be terminated.
    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has started.
    ///
    /// Any post initialization can be performed here, such as writing
    /// to a log file, emmitting metrics.
    ///
    /// Panics in `post_start` follow the supervision strategy.
    fn post_start(&mut self, ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has been stopped.
    fn post_stop(&mut self) {}

    /// Return a supervisor strategy that will be used when handling failed child actors.
    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }

    /// Invoked when an actor receives a system message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `recv` and `sys_recv`.
    fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Sender) {}

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `recv` and `sys_recv`.
    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender);
}

impl<A: Actor + ?Sized> Actor for Box<A> {
    type Msg = A::Msg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).pre_start(ctx);
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).post_start(ctx)
    }

    fn post_stop(&mut self) {
        (**self).post_stop()
    }

    fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
        (**self).sys_recv(ctx, msg, sender)
    }

    fn supervisor_strategy(&self) -> Strategy {
        (**self).supervisor_strategy()
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        (**self).recv(ctx, msg, sender)
    }
}

/// Receive and handle a specific message type
///
/// This trait is typically used in conjuction with the #[actor]
/// attribute macro and implemented for each message type to receive.
///
/// # Examples
///
/// ```
/// # use riker::actors::*;
///
/// #[derive(Clone, Debug)]
/// pub struct Foo;
/// #[derive(Clone, Debug)]
/// pub struct Bar;
/// #[actor(Foo, Bar)] // <-- set our actor to receive Foo and Bar types
/// #[derive(Default)]
/// struct MyActor;
///
/// impl Actor for MyActor {
///     type Msg = MyActorMsg; // <-- MyActorMsg is provided for us
///
///     fn recv(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Self::Msg,
///                 sender: Sender) {
///         self.receive(ctx, msg, sender); // <-- call the respective implementation
///     }
/// }
///
/// impl Receive<Foo> for MyActor {
///     type Msg = MyActorMsg;
///
///     fn receive(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Foo, // <-- receive Foo
///                 sender: Sender) {
///         println!("Received a Foo");
///     }
/// }
///
/// impl Receive<Bar> for MyActor {
///     type Msg = MyActorMsg;
///
///     fn receive(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Bar, // <-- receive Bar
///                 sender: Sender) {
///         println!("Received a Bar");
///     }
/// }
///
/// // main
/// let sys = ActorSystem::new().unwrap();
/// let actor = sys.actor_of::<MyActor>("my-actor").unwrap();
///
/// actor.tell(Foo, None);
/// actor.tell(Bar, None);
/// ```
pub trait Receive<Msg: Message> {
    type Msg: Message;

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Msg, sender: Option<BasicActorRef>);
}

/// The actor trait object
pub type BoxActor<Msg> = Box<dyn Actor<Msg = Msg> + Send>;

/// Supervision strategy
///
/// Returned in `Actor.supervision_strategy`
pub enum Strategy {
    /// Stop the child actor
    Stop,

    /// Attempt to restart the child actor
    Restart,

    /// Escalate the failure to a parent
    Escalate,
}
