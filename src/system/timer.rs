use std::time::{SystemTime, Duration};
use std::sync::mpsc::Sender;

use chrono::{DateTime, Utc};
use config::Config;
use uuid::Uuid;

use crate::protocol::{Message, ActorMsg};
use crate::actor::{ActorRef, Tell};

pub trait Timer {
    type Msg: Message;

    fn schedule<T>(&self,
        initial_delay: Duration,
        interval: Duration,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T)
        -> Uuid
        where T: Into<ActorMsg<Self::Msg>>;

    fn schedule_once<T>(&self,
        delay: Duration,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T)
        -> Uuid
        where T: Into<ActorMsg<Self::Msg>>;

    fn schedule_at_time<T>(&self,
        time: DateTime<Utc>,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T)
        -> Uuid
        where T: Into<ActorMsg<Self::Msg>>;

    fn cancel_schedule(&self, id: Uuid);
}

pub trait TimerFactory {
    type Msg: Message;

    fn new(config: &Config, debug: bool) -> Sender<Job<Self::Msg>>;
}

pub enum Job<Msg: Message> {
    Once(OnceJob<Msg>),
    Repeat(RepeatJob<Msg>),
    Cancel(Uuid),
}

pub struct OnceJob<Msg: Message> {
    pub id: Uuid,
    pub send_at: SystemTime,
    pub receiver: ActorRef<Msg>,
    pub sender: Option<ActorRef<Msg>>,
    pub msg: ActorMsg<Msg>,
}

impl<Msg: Message> OnceJob<Msg> {
    pub fn send(self) {
        let _ = self.receiver.tell(self.msg, self.sender); // TODO add sender 
    }
}

pub struct RepeatJob<Msg: Message> {
    pub id: Uuid,
    pub send_at: SystemTime,
    pub interval: Duration,
    pub receiver: ActorRef<Msg>,
    pub sender: Option<ActorRef<Msg>>,
    pub msg: ActorMsg<Msg>,
}

impl<Msg: Message> RepeatJob<Msg> {
    pub fn send(&self) {
        let _ = self.receiver.tell(self.msg.clone(), self.sender.clone()); // TODO add sender
    }
}