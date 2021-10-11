#![deny(clippy::all)]
// #![deny(clippy::pedantic)]
// #![deny(clippy::nursery)]
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::from_over_into)]
#![allow(clippy::new_without_default)]
#![forbid(unsafe_code)]

mod validate;

pub mod actor;
pub mod kernel;
pub mod system;
mod tokio_backend;
mod config;

use std::any::Any;
use std::fmt;
use std::fmt::Debug;

use crate::actor::BasicActorRef;

pub use self::config::{Config, load_config};

/// Wraps message and sender
#[derive(Debug, Clone)]
pub struct Envelope<T: Message> {
    pub sender: Option<BasicActorRef>,
    pub msg: T,
}

pub trait Message: Debug + Clone + Send + 'static {}
impl<T: Debug + Clone + Send + 'static> Message for T {}

pub struct AnyMessage {
    pub one_time: bool,
    pub msg: Option<Box<dyn Any + Send>>,
}

pub struct DowncastAnyMessageError;

impl AnyMessage {
    pub fn new<T>(msg: T, one_time: bool) -> Self
    where
        T: Any + Message,
    {
        Self {
            one_time,
            msg: Some(Box::new(msg)),
        }
    }

    pub fn take<T>(&mut self) -> Result<T, DowncastAnyMessageError>
    where
        T: Any + Message,
    {
        if self.one_time {
            match self.msg.take() {
                Some(m) => {
                    if m.is::<T>() {
                        Ok(*m.downcast::<T>().unwrap())
                    } else {
                        Err(DowncastAnyMessageError)
                    }
                }
                None => Err(DowncastAnyMessageError),
            }
        } else {
            match self.msg.as_ref() {
                Some(m) if m.is::<T>() => Ok(m.downcast_ref::<T>().cloned().unwrap()),
                Some(_) => Err(DowncastAnyMessageError),
                None => Err(DowncastAnyMessageError),
            }
        }
    }
}

impl Clone for AnyMessage {
    fn clone(&self) -> Self {
        panic!("Can't clone a message of type `AnyMessage`");
    }
}

impl Debug for AnyMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AnyMessage")
    }
}

pub mod actors {
    pub use crate::actor::{
        Context, Actor, actor, Receive, CreateError,
        ActorRef, ActorRefFactory, ActorReference, BasicActorRef, BoxedTell, Sender, Tell,
        channel, All, Channel, ChannelMsg, ChannelRef, DLChannelMsg, DeadLetter, EventsChannel,
        Publish, Subscribe, SubscribeWithResponse, SubscribedResponse, SysTopic, Topic, Unsubscribe, UnsubscribeAll,
        ActorArgs, ActorFactory, ActorFactoryArgs, ActorProducer, BoxActorProd, Props,
        ActorPath, ActorUri,
    };
    pub use crate::system::{
        ActorSystem, ScheduleId, SystemBuilder, SystemEvent, SystemMsg, Timer,
        ActorSystemBackend, SendingBackend,
    };
    pub use crate::tokio_backend::ActorSystemBackendTokio;
    pub use crate::{AnyMessage, Message};
}
