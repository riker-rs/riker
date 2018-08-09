extern crate riker;
extern crate riker_default;
#[macro_use]
extern crate riker_testkit;

use riker::actors::*;
use riker_default::DefaultModel;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

type TestProbe = ChannelProbe<(), ()>;

#[derive(Clone, Debug)]
enum TestMsg {
    Probe(TestProbe),
    Message(String),
}

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

struct DumbActor;

impl DumbActor {
    fn new() -> BoxActor<TestMsg> {
        Box::new(DumbActor)
    }
}

impl Actor for DumbActor {
    type Msg = TestMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
}


// *** Publish test ***
struct SubscribeActor {
    probe: Option<TestProbe>,
    topic: Topic,
}

impl SubscribeActor {
    fn actor(topic: Topic) -> BoxActor<TestMsg> {
        let actor = SubscribeActor {
            probe: None,
            topic
        };

        Box::new(actor)
    }
}

impl Actor for SubscribeActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let msg = ChannelMsg::Subscribe(self.topic.clone(), ctx.myself());
        ctx.system.default_stream().tell(msg, None);
    }

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Probe(probe) => {
                probe.event(());
                self.probe = Some(probe);
            }
            TestMsg::Message(_) => self.probe.event(())
        }
    }
}

// lazy_static! {
//     static ref MYTOPIC: Topic = {
//         Topic("my_topic".to_string())
//     };
// }
struct MyTopic;

impl From<MyTopic> for Topic {
    fn from(_my: MyTopic) -> Self {
        Topic::from("my_topic")
    }
}

#[test]
fn publish() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new_args(Box::new(SubscribeActor::actor), MyTopic.into());
    let actor = system.actor_of(props, "sub-actor").unwrap();
    
    let (probe, listen) = probe();
    actor.tell(TestMsg::Probe(probe), None);
    
    // wait for the probe to arrive at the actor before publishing message
    listen.recv();
    
    let msg = TestMsg::Message("hello world!".to_string());
    let msg = ChannelMsg::Publish(MyTopic.into(), msg);
    system.default_stream().tell(msg, None);

    p_assert_eq!(listen, ());
}

#[test]
fn publish_subscribe_all() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    // Create actor that subscribes to all topics ("*") on the channel 
    let props = Props::new_args(Box::new(SubscribeActor::actor), "*".into());
    let actor = system.actor_of(props, "sub-actor").unwrap();
    
    let (probe, listen) = probe();
    actor.tell(TestMsg::Probe(probe), None);
    
    // wait for the probe to arrive at the actor before publishing message
    listen.recv();
    
    // Publish to "topic-1"
    let msg1 = TestMsg::Message("hello world!".to_string());
    let msg1 = ChannelMsg::Publish("topic-1".into(), msg1.clone());
    system.default_stream().tell(msg1, None);

    // Publish to "topic-2"
    let msg2 = TestMsg::Message("hello world!".to_string());
    let msg2 = ChannelMsg::Publish("topic-2".into(), msg2.clone());
    system.default_stream().tell(msg2, None);

    // Publish to "topic-3"
    let msg3 = TestMsg::Message("hello world!".to_string());
    let msg3 = ChannelMsg::Publish("topic-3".into(), msg3.clone());
    system.default_stream().tell(msg3, None);

    // Expecting three probe events
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}


// *** Event stream test ***
struct EventSubActor {
    probe: Option<TestProbe>,
}

impl EventSubActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = EventSubActor {
            probe: None
        };

        Box::new(actor)
    }
}

impl Actor for EventSubActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe to the actor created event topic
        let msg = ChannelMsg::Subscribe(SysTopic::ActorCreated.into(), ctx.myself());
        ctx.system.event_stream().tell(msg, None);
    }

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Probe(probe) => {
                probe.event(());
                self.probe = Some(probe);
            }
            _ => {}
        }
    }

    fn system_receive(&mut self, _: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, _sender: Option<ActorRef<Self::Msg>>) {
        if let SystemMsg::Event(evt) = msg {
            match evt {
                SystemEvent::ActorCreated(_) => {
                    self.probe.event(())
                }
                _ => {}
            }
        }
    }
}

#[test]
fn event_stream() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(EventSubActor::new));
    let actor = system.actor_of(props, "event-subscriber").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestMsg::Probe(probe), None);
    
    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    let props = Props::new(Box::new(DumbActor::new));
    system.actor_of(props, "dumb-actor").unwrap();

    p_assert_eq!(listen, ());
}


// *** Dead letters test ***
struct DeadLettersActor {
    probe: Option<TestProbe>,
}

impl DeadLettersActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = DeadLettersActor {
            probe: None
        };

        Box::new(actor)
    }
}

impl Actor for DeadLettersActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe to dead_letters
        let msg = ChannelMsg::Subscribe(All.into(), ctx.myself());
        ctx.system.dead_letters().tell(msg, None);
    }

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        if let TestMsg::Probe(probe) = msg {
            probe.event(()); // notify listen then probe has been received.
            self.probe = Some(probe);
        }
    }

    fn other_receive(&mut self, _: &Context<Self::Msg>, msg: ActorMsg<Self::Msg>, _: Option<ActorRef<Self::Msg>>) {
        if let ActorMsg::DeadLetter(dl) = msg {
            println!("DeadLetter: {} => {} ({:?})", dl.sender, dl.recipient, dl.msg);
            self.probe.event(());
        }
    }
}

#[test]
fn dead_letters() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(DeadLettersActor::new));
    let actor = system.actor_of(props, "dl-subscriber").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestMsg::Probe(probe), None);
    
    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    let props = Props::new(Box::new(DumbActor::new));
    let dumb = system.actor_of(props, "dumb-actor").unwrap();
    
    // immediately stop the actor and attempt to send a message
    system.stop(&dumb);
    std::thread::sleep(std::time::Duration::from_secs(1));
    dumb.tell(TestMsg::Message("hello".to_string()), None);

    p_assert_eq!(listen, ());
}