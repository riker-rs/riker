pub(crate) mod actor_cell;
pub(crate) mod actor_ref;
pub(crate) mod channel;
pub(crate) mod macros;
pub(crate) mod props;
pub(crate) mod uri;

use std::{error, fmt};

use crate::validate::InvalidName;

// Public API (plus the pub data types in this file)
pub use self::{
    actor_cell::Context,
    actor_ref::{
        ActorRef, ActorRefFactory, ActorReference, BasicActorRef, BoxedTell, Sender, Tell,
    },
    channel::{
        channel, All, Channel, ChannelMsg, ChannelRef, DLChannelMsg, DeadLetter, EventsChannel,
        Publish, Subscribe, SubscribeWithResponse, SubscribedResponse, SysTopic, Topic,
        Unsubscribe, UnsubscribeAll,
    },
    macros::actor,
    props::{ActorArgs, ActorFactory, ActorFactoryArgs, ActorProducer, BoxActorProd, Props},
    uri::{ActorPath, ActorUri},
};

use crate::{system::SystemMsg, Message};

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

impl<T> fmt::Display for MsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("The actor does not exist. It may have been terminated")
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

impl<T> fmt::Display for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Option<ActorRef> is None")
    }
}

impl<T> fmt::Debug for TryMsgError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// Error type when an actor fails to start during `actor_of`.
#[derive(Debug)]
pub enum CreateError {
    Panicked,
    System,
    InvalidName(String),
    AlreadyExists(ActorPath),
}

impl error::Error for CreateError {}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Panicked => {
                f.write_str("Failed to create actor. Cause: Actor panicked while starting")
            }
            Self::System => f.write_str("Failed to create actor. Cause: System failure"),
            Self::InvalidName(ref name) => f.write_str(&format!(
                "Failed to create actor. Cause: Invalid actor name ({})",
                name
            )),
            Self::AlreadyExists(ref path) => f.write_str(&format!(
                "Failed to create actor. Cause: An actor at the same path already exists ({})",
                path
            )),
        }
    }
}

impl From<InvalidName> for CreateError {
    fn from(err: InvalidName) -> CreateError {
        CreateError::InvalidName(err.name)
    }
}

/// Error type when an actor fails to restart.
pub struct RestartError;

impl fmt::Display for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Failed to restart actor. Cause: Actor panicked while starting")
    }
}

impl fmt::Debug for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

#[allow(unused_variables)]
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

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
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
/// # use tezedge_actor_system::actors::*;
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
/// fn main() {
///     let sys = ActorSystem::new().unwrap();
///     let actor = sys.actor_of::<MyActor>("my-actor").unwrap();
///
///     actor.tell(Foo, None);
///     actor.tell(Bar, None);
///     sys.shutdown()
/// }
/// ```
pub trait Receive<Msg: Message> {
    type Msg: Message;

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Msg, sender: Sender);
}
