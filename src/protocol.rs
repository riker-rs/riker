use std::fmt::Debug;
use std::io::ErrorKind;

use bytes::Bytes;

use crate::actor::{ActorRef, BoxActorProd};
use crate::actors::{Evt, Topic};
use crate::system::{LogEntry, IoType, IoAddress};

/// Enum used to store messages in an actor's mailbox
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum Enqueued<T: Message> {
    ActorMsg(Envelope<T>),
    SystemMsg(SystemEnvelope<T>)
}

/// Wraps message and sender
#[derive(Debug, Clone)]
pub struct Envelope<T: Message> {
    pub sender: Option<ActorRef<T>>,
    pub msg: ActorMsg<T>,
}

/// Wraps system message and sender
#[derive(Clone, Debug)]
pub struct SystemEnvelope<T: Message> {
    pub sender: Option<ActorRef<T>>,
    pub msg: SystemMsg<T>,
}

/// Standard message type. All actor messages are `ActorMsg`
#[derive(Debug, Clone)]
pub enum ActorMsg<Msg: Message> {
    /// User message type
    User(Msg),

    /// Channel messages
    Channel(ChannelMsg<Msg>),

    /// IO messages (IoManager)
    IO(IOMsg<Msg>),

    /// Event sourcing messages
    ES(ESMsg<Msg>),

    /// CQRS messages
    CQ(CQMsg<Msg>),

    /// Request actor info
    Identify,

    /// Response to Identify
    Info(Info),

    /// Dead letter messages
    DeadLetter(Box<DeadLetter<Msg>>),

    /// A utility message for user to schedule actor execution
    Tick,

    // ...
}

#[derive(Debug, Clone)]
pub enum ChannelMsg<Msg: Message> {
    /// Publish message
    Publish(Topic, Msg),

    /// Publish system event
    PublishEvent(SystemEvent<Msg>),

    /// Publish dead letter
    PublishDeadLetter(Box<DeadLetter<Msg>>),

    /// Subscribe given `ActorRef` to a topic on a channel
    Subscribe(Topic, ActorRef<Msg>),

    /// Unsubscribe the given `ActorRef` from a topic on a channel
    Unsubscribe(Topic, ActorRef<Msg>),

    /// Unsubscribe the given `ActorRef` from all topics on a channel
    UnsubscribeAll(ActorRef<Msg>),
}

impl<Msg: Message> Into<ActorMsg<Msg>> for ChannelMsg<Msg> {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::Channel(self)
    }
}

#[derive(Debug, Clone)]
pub enum IOMsg<Msg: Message> {
    /// Register a connection manager for the given `IoType`
    Manage(IoType, BoxActorProd<Msg>),

    /// Bind on an IO type, e.g. TCP Socket
    Bind(IoType, IoAddress, ActorRef<Msg>),

    /// Received when an IO type is bound, e.g. TCP Socket
    Bound(IoAddress),

    /// Unbind an IO type, e.g. TCP Socket
    Unbind,

    /// Received when an IO type is unbound
    Unbound(IoAddress),

    /// Connect to an `IoAddress`, e.g. TCP/IP Address
    Connect(IoType, IoAddress),

    /// Received when an IO type is connected, e.g. TCP/IP Address
    Connected(IoAddress, IoAddress),

    /// Register given actor to receive data on a connected `IoAddress`
    Register(ActorRef<Msg>),

    /// Close the IO resource, e.g. disconnect from TCP/IP Address
    Close,

    /// Received when an IO resource is closed
    Closed,
    
    /// IO resource is ready to read or write
    Ready,

    /// Write given bytes to IO resource
    Write(Bytes),

    /// Currently not used
    Send(IoType),

    /// ???
    TryRead,

    /// Received when IO resource reads bytes
    Received(Bytes),

    /// Flush any cached data
    Flush,

    /// Received when an IO operation failed, e.g. `Bind` and `Connect`
    Failed(ErrorKind),
}

impl<Msg: Message> Into<ActorMsg<Msg>> for IOMsg<Msg> {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::IO(self)
    }
}

#[derive(Debug, Clone)]
pub enum ESMsg<Msg: Message> {
    /// Persist given Evt to the event store. (Event to store, Unique ID, Keyspace, Optional Sender)
    Persist(Evt<Msg>, String, String, Option<ActorRef<Msg>>),

    /// Load all events from the event store. (Unique ID, Keyspace)
    Load(String, String),

    /// Received when loading events
    LoadResult(Vec<Msg>),
}

impl<Msg: Message> Into<ActorMsg<Msg>> for ESMsg<Msg> {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::ES(self)
    }
}

#[derive(Clone, Debug)]
pub enum CQMsg<Msg: Message> {
    /// CQRS command message
    Cmd(String, Msg),
}

impl<Msg: Message> Into<ActorMsg<Msg>> for CQMsg<Msg> {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::CQ(self)
    }
}

/// Message type to request actor info
pub struct Identify;

impl<Msg: Message> Into<ActorMsg<Msg>> for Identify {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::Identify
    }
}

/// Message type received in response to `Identify`
#[derive(Debug, Clone)]
pub struct Info; // this will be expanded to a full struct, later, containing stats on the actor

impl<Msg: Message> Into<ActorMsg<Msg>> for Info {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::Info(self)
    }
}

#[derive(Clone, Debug)]
pub enum SystemMsg<Msg: Message> {
    ActorInit,
    ActorCmd(ActorCmd),
    Event(SystemEvent<Msg>),
    Failed(ActorRef<Msg>),
    Persisted(Msg, Option<ActorRef<Msg>>),
    Replay(Vec<Msg>),
    Log(LogEntry),
}

#[derive(Clone, Debug)]
pub enum ActorCmd {
    Stop,
    Restart,
}

#[derive(Clone, Debug)]
pub enum SystemEvent<Msg: Message> {
    /// An actor was terminated
    ActorTerminated(ActorRef<Msg>),

    /// An actor was restarted
    ActorRestarted(ActorRef<Msg>),

    /// An actor was started
    ActorCreated(ActorRef<Msg>),
}

#[derive(Clone, Debug)]
pub enum SystemEventType {
    ActorTerminated,
    ActorRestarted,
    ActorCreated,
}

#[derive(Clone, Debug)]
pub struct DeadLetter<Msg: Message> {
    pub msg: ActorMsg<Msg>,
    pub sender: String,
    pub recipient: String,
}

impl<Msg: Message> Into<ActorMsg<Msg>> for DeadLetter<Msg> {
    fn into(self) -> ActorMsg<Msg> {
        ActorMsg::DeadLetter(Box::new(self))
    }
}

// implement Into<ActorMsg<Msg>> for common types String, u32
impl Into<ActorMsg<String>> for String {
    fn into(self) -> ActorMsg<String> {
        ActorMsg::User(self)
    }
}

impl<'a> Into<ActorMsg<String>> for &'a str {
    fn into(self) -> ActorMsg<String> {
        ActorMsg::User(self.to_string())
    }
}

impl Into<ActorMsg<u32>> for u32 {
    fn into(self) -> ActorMsg<u32> {
        ActorMsg::User(self)
    }
}

unsafe impl<T: Message> Send for Envelope<T> {}

pub trait Message: Debug + Clone + Send + Into<ActorMsg<Self>> + 'static {}
impl<T: Debug + Clone + Send + Into<ActorMsg<Self>> + 'static> Message for T {}
