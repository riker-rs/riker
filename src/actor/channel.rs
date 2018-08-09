#![allow(unused_variables)]

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use protocol::{Message, ActorMsg, SystemMsg, ChannelMsg, SystemEvent, DeadLetter};
use ::actor::{Actor, BoxActor, Props, BoxActorProd, ActorRef, Context, Tell, SysTell};

type Subs<Msg> = HashMap<Topic, Vec<ActorRef<Msg>>>;

/// A specialized actor for providing Publish/Subscribe capabilities to users.
/// 
/// It is a common actor pattern to provide pub/sub features to other actors
/// especially in cases where choreography (instead of orchestration) is used.
/// [See: Service Choreography](https://en.wikipedia.org/wiki/Service_choreography)
/// 
/// A channel can be started as you would any other actor. A channel expects
/// `ChannelMsg` messages.
/// 
/// To publish a message to a channel you send the channel a `ChannelMsg::Publish`
/// message containing the topic and the message to publish.
/// 
/// A published message is cloned and sent to each subscriber to the channel where
/// the topic matches.
/// 
/// To subscribe to a channel you send the channel a `ChannelMsg::Subscribe`
/// message containing the topic to subscribe to and an `ActorRef` of the
/// subscriber (e.g. `.myself()`).
/// 
/// Since channels are actors themselves they provide excellent lightweight
/// facilitators of distributing data among actors that are working together to
/// complete a single goal or interaction (even short lived interactions).
/// 
/// # Examples
/// 
/// ```
/// # extern crate riker;
/// # extern crate riker_default;
/// 
/// # use riker::actors::*;
/// # use riker_default::DefaultModel;
/// 
/// use riker::actors::ChannelMsg::*;
/// 
/// struct MyActor;
/// 
/// impl Actor for MyActor {
///     type Msg = String;
///
///     fn receive(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Self::Msg,
///                 sender: Option<ActorRef<Self::Msg>>) {
///         println!("Received msg {:?}", msg);
///     }
/// }
/// 
/// impl MyActor {
///     fn actor() -> BoxActor<String> {
///         Box::new(MyActor)
///     }
/// }
/// 
/// // main
/// let model: DefaultModel<String> = DefaultModel::new();
/// let sys = ActorSystem::new(&model).unwrap();
///
/// // start two instances of MyActor
/// let props = Props::new(Box::new(MyActor::actor));
/// let sub1 = sys.actor_of(props.clone(), "sub1").unwrap();
/// let sub2 = sys.actor_of(props, "sub2").unwrap();
/// 
/// // start a channel
/// let chan = sys.actor_of(Channel::props(), "my-channel").unwrap();
/// 
/// // subscribe actors to channel
/// chan.tell(Subscribe("my-topic".into(), sub1), None);
/// chan.tell(Subscribe("my-topic".into(), sub2), None);
/// 
/// // publish a message
/// let msg = Publish("my-topic".into(), "Remember the cant!".into());
/// chan.tell(msg, None);
/// ```
pub struct Channel<Msg: Message> {
    event_stream: Option<ActorRef<Msg>>,
    subs: Subs<Msg>,
}

impl<Msg: Message> Channel<Msg> {
    pub fn new(event_stream: Option<ActorRef<Msg>>) -> BoxActor<Msg> {
        let actor = Channel {
            event_stream,
            subs: Subs::new()
        };

        Box::new(actor)
    }

    pub fn props() -> BoxActorProd<Msg> {
        Props::new_args(Box::new(Channel::new), None)
    }

    fn handle_channel_msg(&mut self, msg: ChannelMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        match msg {
            ChannelMsg::Publish(topic, msg) => self.publish(&topic, msg, sender),
            ChannelMsg::Subscribe(topic, actor) => self.subscribe(topic, actor),
            ChannelMsg::Unsubscribe(topic, actor) => self.unsubscribe(&topic, &actor),
            ChannelMsg::UnsubscribeAll(actor) => self.unsubscribe_from_all(&actor),
            _ => {}
        }
    }

    fn publish(&mut self, topic: &Topic, msg: Msg, sender: Option<ActorRef<Msg>>) {
        // send message to actors subscribed to all topics
        if let Some(subs) = self.subs.get(&All.into()) {
            for sub in subs.iter() {
                let msg = msg.clone();
                sub.tell(msg, sender.clone());
            }
        }

        // send message to actors subscribed to the topic
        if let Some(subs) = self.subs.get(topic) {
            for sub in subs.iter() {
                let msg = msg.clone();
                sub.tell(msg, sender.clone());
            }
        }
    }

    fn subscribe(&mut self, topic: Topic, actor: ActorRef<Msg>) {
        if self.subs.contains_key(&topic) {
            self.subs.get_mut(&topic).unwrap().push(actor);
        } else {
            self.subs.insert(topic.clone(), vec![actor]);
        }
    }

    fn unsubscribe(&mut self, topic: &Topic, actor: &ActorRef<Msg>) {
        // Nightly only: self.subs.get(msg_type).unwrap().remove_item(actor);
        if self.subs.contains_key(topic) {
            if let Some(pos) = self.subs.get(topic).unwrap().iter().position(|x| x == actor) {
                self.subs.get_mut(topic).unwrap().remove(pos);
            }
        }
    }

    fn unsubscribe_from_all(&mut self, actor: &ActorRef<Msg>) {
        //TODO using .clone here to work around multiple borrows on self. Need to find a better way.
        let subs = self.subs.clone();

        for topic in subs.keys() {
            self.unsubscribe(&topic, &actor);
        }
    }
}

impl<Msg: Message> Actor for Channel<Msg> {
    type Msg = Msg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let msg = ChannelMsg::Subscribe(SysTopic::ActorTerminated.into(), ctx.myself.clone());

        match self.event_stream {
            Some(ref es) => es.tell(msg, None),
            None => ctx.system.event_stream().tell(msg, None)
        };
    }

    fn other_receive(&mut self, _: &Context<Msg>, msg: ActorMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        match msg {
            ActorMsg::Channel(msg) => self.handle_channel_msg(msg, sender),
            _ => {} //TODO send to dead letters?
        }
    }

    fn system_receive(&mut self, _: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {
        if let SystemMsg::Event(evt) = msg {
            match evt {
                SystemEvent::ActorTerminated(actor) => {
                    self.unsubscribe_from_all(&actor)
                }
                _ => {}
            }
        }
    }

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {}
}

/// A specialized actor for providing Publish/Subscribe capabilities for system messages.
pub struct SystemChannel<Msg: Message> {
    subs: Subs<Msg>,
}

impl<Msg: Message> SystemChannel<Msg> {
    pub fn new() -> BoxActor<Msg> {
        let actor = SystemChannel {
            subs: Subs::new()
        };

        Box::new(actor)
    }

    fn handle_channel_msg(&mut self, msg: ChannelMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        match msg {
            ChannelMsg::PublishEvent(evt) => self.publish_event(evt, sender),
            ChannelMsg::PublishDeadLetter(msg) => self.publish_deadleatter(*msg),
            ChannelMsg::Subscribe(topic, actor) => self.subscribe(topic, actor),
            ChannelMsg::Unsubscribe(topic, actor) => self.unsubscribe(&topic, &actor),
            ChannelMsg::UnsubscribeAll(actor) => self.unsubscribe_from_all(&actor),
            _ => {}
        }
    }

    fn publish_event(&mut self, evt: SystemEvent<Msg>, sender: Option<ActorRef<Msg>>) {
        // send message to actors subscribed to all topics
        if let Some(subs) = self.subs.get(&All.into()) {
            for sub in subs.iter() {
                let msg = SystemMsg::Event(evt.clone());
                sub.sys_tell(msg, sender.clone());
            }
        }

        // send message to actors subscribed to the topic
        if let Some(subs) = self.subs.get(&Topic::from(&evt)) {
            for sub in subs.iter() {
                let msg = SystemMsg::Event(evt.clone());
                sub.sys_tell(msg, sender.clone());
            }
        }  
    }

    fn publish_deadleatter(&mut self, msg: DeadLetter<Msg>) {
        if let Some(subs) = self.subs.get(&All.into()) {
            for sub in subs.iter() {
                sub.tell(msg.clone(), None);
            }
        }
    }

    fn subscribe(&mut self, topic: Topic, actor: ActorRef<Msg>) {
        if self.subs.contains_key(&topic) {
            self.subs.get_mut(&topic).unwrap().push(actor);
        } else {
            self.subs.insert(topic, vec![actor]);
        }
    }

    fn unsubscribe(&mut self, topic: &Topic, actor: &ActorRef<Msg>) {
        // Nightly only: self.subs.get(msg_type).unwrap().remove_item(actor);
        if self.subs.contains_key(topic) {
            if let Some(pos) = self.subs.get(topic).unwrap().iter().position(|x| x == actor) {
                self.subs.get_mut(topic).unwrap().remove(pos);
            }
        }
    }

    fn unsubscribe_from_all(&mut self, actor: &ActorRef<Msg>) {
        //TODO using .clone here to work around multiple borrows on self. Need to find a better way.
        let subs = self.subs.clone();

        for topic in subs.keys() {
            self.unsubscribe(&topic, &actor);
        }
    }
}

impl<Msg: Message> Actor for SystemChannel<Msg> {
    type Msg = Msg;

    fn pre_start(&mut self, ctx: &Context<Msg>) {
        let msg = ChannelMsg::Subscribe(SysTopic::ActorTerminated.into(), ctx.myself.clone());
        ctx.myself.tell(msg, None);
    }

    fn other_receive(&mut self, _: &Context<Msg>, msg: ActorMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        match msg {
            ActorMsg::Channel(msg) => self.handle_channel_msg(msg, sender),
            _ => {} //TODO send to dead letters?
        }
    }

    fn system_receive(&mut self, _: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {
        if let SystemMsg::Event(evt) = msg {
            match evt {
                SystemEvent::ActorTerminated(actor) => {
                    self.unsubscribe_from_all(&actor)
                }
                _ => {}
            }
        }
    }

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {}
}

/// Topics allow channel subscribers to filter messages by interest
/// 
/// When publishing a message to a channel a Topic is provided.
#[derive(Clone, Debug, PartialEq)]
pub struct Topic(String);

impl<'a> From<&'a str> for Topic {
    fn from(topic: &str) -> Self {
        Topic(topic.to_string())
    }
}

impl From<String> for Topic {
    fn from(topic: String) -> Self {
        Topic(topic.to_string())
    }
}

impl<'a, Msg: Message> From<&'a SystemEvent<Msg>> for Topic {
    fn from(evt: &SystemEvent<Msg>) -> Self {
        match evt {
            &SystemEvent::ActorCreated(_) => Topic::from("actor.created"),
            &SystemEvent::ActorTerminated(_) => Topic::from("actor.terminated"),
            &SystemEvent::ActorRestarted(_) => Topic::from("actor.restarted")
        }
    }
}

impl Eq for Topic {}

impl Hash for Topic {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// A channel topic representing all topics `*`
pub struct All;

impl From<All> for Topic {
    fn from(all: All) -> Self {
        Topic::from("*")
    }
}

/// System topics used by the `event_stream` channel
pub enum SysTopic {
    ActorCreated,
    ActorTerminated,
    ActorRestarted,
}

impl From<SysTopic> for Topic {
    fn from(evt: SysTopic) -> Self {
        match evt {
            SysTopic::ActorCreated => Topic::from("actor.created"),
            SysTopic::ActorTerminated => Topic::from("actor.terminated"),
            SysTopic::ActorRestarted => Topic::from("actor.restarted")
        }
    }
}

// TODO update to use actorrefs
pub fn dead_letter<Msg: Message>(dl: &ActorRef<Msg>, sender: Option<String>, recipient: String, msg: ActorMsg<Msg>) {
    let dead_letter = DeadLetter {
        sender: sender.unwrap_or("[unknown sender]".to_string()),
        recipient: recipient,
        msg: msg
    };

    let dead_letter = Box::new(dead_letter);
    let dead_letter = ActorMsg::Channel(ChannelMsg::PublishDeadLetter(dead_letter));

    dl.tell(dead_letter, None);
}

#[derive(Clone)]
pub struct SysChannels<Msg: Message> {
    pub event_stream: ActorRef<Msg>,
    pub dead_letters: ActorRef<Msg>,
}
