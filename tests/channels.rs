#[macro_use]
extern crate riker_testkit;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};
use async_trait::async_trait;
use futures::executor::block_on;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[derive(Clone, Debug)]
pub struct SomeMessage;

// *** Publish test ***
#[actor(TestProbe, SomeMessage)]
struct Subscriber {
    probe: Option<TestProbe>,
    chan: ChannelRef<SomeMessage>,
    topic: Topic,
}

impl Subscriber {
    fn actor((chan, topic): (ChannelRef<SomeMessage>, Topic)) -> Self {
        Subscriber {
            probe: None,
            chan,
            topic,
        }
    }

    fn props(chan: ChannelRef<SomeMessage>, topic: Topic) -> BoxActorProd<Subscriber> {
        Props::new_args(Subscriber::actor, (chan, topic))
    }
}

#[async_trait]
impl Actor for Subscriber {
    type Msg = SubscriberMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Box::new(ctx.myself());
        self.chan.tell(
            Subscribe {
                actor: sub,
                topic: self.topic.clone(),
            },
            None,
        ).await;
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<TestProbe> for Subscriber {
    type Msg = SubscriberMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

#[async_trait]
impl Receive<SomeMessage> for Subscriber {
    type Msg = SubscriberMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: SomeMessage, _sender: Sender) {
        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn channel_publish() {
    let sys = block_on(ActorSystem::new()).unwrap();

    // Create the channel we'll be using
    let chan: ChannelRef<SomeMessage> = block_on(channel("my-chan", &sys)).unwrap();

    // The topic we'll be publishing to. Endow our subscriber test actor with this.
    // On Subscriber's pre_start it will subscribe to this channel+topic
    let topic = Topic::from("my-topic");
    let sub = block_on(sys
        .actor_of(Subscriber::props(chan.clone(), topic.clone()), "sub-actor"))
        .unwrap();

    let (probe, listen) = probe();
    block_on(sub.tell(TestProbe(probe), None));

    // wait for the probe to arrive at the actor before publishing message
    listen.recv();

    // Publish a test message
    block_on(chan.tell(
        Publish {
            msg: SomeMessage,
            topic: topic,
        },
        None,
    ));

    p_assert_eq!(listen, ());
}

#[test]
fn channel_publish_subscribe_all() {
    let sys = block_on(ActorSystem::new()).unwrap();

    // Create the channel we'll be using
    let chan: ChannelRef<SomeMessage> = block_on(channel("my-chan", &sys)).unwrap();

    // The '*' All topic. Endow our subscriber test actor with this.
    // On Subscriber's pre_start it will subscribe to all topics on this channel.
    let topic = Topic::from("*");
    let sub = block_on(sys
        .actor_of(Subscriber::props(chan.clone(), topic.clone()), "sub-actor"))
        .unwrap();

    let (probe, listen) = probe();
    block_on(sub.tell(TestProbe(probe), None));

    // wait for the probe to arrive at the actor before publishing message
    listen.recv();

    // Publish a test message to topic "topic-1"
    block_on(chan.tell(
        Publish {
            msg: SomeMessage,
            topic: "topic-1".into(),
        },
        None,
    ));

    // Publish a test message to topic "topic-2"
    block_on(chan.tell(
        Publish {
            msg: SomeMessage,
            topic: "topic-2".into(),
        },
        None,
    ));

    // Publish a test message to topic "topic-3"
    block_on(chan.tell(
        Publish {
            msg: SomeMessage,
            topic: "topic-3".into(),
        },
        None,
    ));

    // Expecting three probe events
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}

#[derive(Clone, Debug)]
pub struct Panic;

#[actor(Panic, SomeMessage)]
struct DumbActor;

impl DumbActor {
    fn new() -> Self {
        DumbActor
    }
}

#[async_trait]
impl Actor for DumbActor {
    type Msg = DumbActorMsg;

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<Panic> for DumbActor {
    type Msg = DumbActorMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

#[async_trait]
impl Receive<SomeMessage> for DumbActor {
    type Msg = DumbActorMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: SomeMessage, _sender: Sender) {

        // Intentionally left blank
    }
}

// We must wrap SystemEvent in a type defined in this test crate
// so we can implement traits on it
#[derive(Clone, Debug)]
struct SysEvent(SystemEvent);

// *** Event stream test ***
#[actor(TestProbe, SystemEvent)]
struct EventSubscriber {
    probe: Option<TestProbe>,
}

impl EventSubscriber {
    fn new() -> Self {
        EventSubscriber { probe: None }
    }

    fn props() -> BoxActorProd<EventSubscriber> {
        Props::new(EventSubscriber::new)
    }
}

#[async_trait]
impl Actor for EventSubscriber {
    type Msg = EventSubscriberMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe
        let sub = Box::new(ctx.myself());
        ctx.system.sys_events().tell(
            Subscribe {
                actor: sub,
                topic: "*".into(),
            },
            None,
        ).await;
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }

    async fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Sender) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender).await;
        }
    }
}

#[async_trait]
impl Receive<TestProbe> for EventSubscriber {
    type Msg = EventSubscriberMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

#[async_trait]
impl Receive<SystemEvent> for EventSubscriber {
    type Msg = EventSubscriberMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: SystemEvent, _sender: Sender) {
        match msg {
            SystemEvent::ActorCreated(created) => {
                if created.actor.path() == "/user/dumb-actor" {
                    self.probe.as_ref().unwrap().0.event(())
                }
            }
            SystemEvent::ActorRestarted(restarted) => {
                if restarted.actor.path() == "/user/dumb-actor" {
                    self.probe.as_ref().unwrap().0.event(())
                }
            }
            SystemEvent::ActorTerminated(terminated) => {
                if terminated.actor.path() == "/user/dumb-actor" {
                    self.probe.as_ref().unwrap().0.event(())
                }
            }
        }
    }
}

#[test]
fn channel_system_events() {
    let sys = block_on(ActorSystem::new()).unwrap();

    let actor = block_on(sys.actor_of(EventSubscriber::props(), "event-sub")).unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before attempting
    // create, restart and stop
    listen.recv();

    // Create an actor
    let props = Props::new(DumbActor::new);
    let dumb = block_on(sys.actor_of(props, "dumb-actor")).unwrap();
    // ActorCreated event was received
    p_assert_eq!(listen, ());

    // Force restart of actor
    block_on(dumb.tell(Panic, None));
    // ActorRestarted event was received
    p_assert_eq!(listen, ());

    // Terminate actor
    block_on(sys.stop(&dumb));
    // ActorTerminated event was receive
    p_assert_eq!(listen, ());
}

// *** Dead letters test ***
#[actor(TestProbe, DeadLetter)]
struct DeadLetterSub {
    probe: Option<TestProbe>,
}

impl DeadLetterSub {
    fn new() -> Self {
        DeadLetterSub { probe: None }
    }

    fn props() -> BoxActorProd<DeadLetterSub> {
        Props::new(DeadLetterSub::new)
    }
}

#[async_trait]
impl Actor for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe to dead_letters
        let sub = Box::new(ctx.myself());
        ctx.system.dead_letters().tell(
            Subscribe {
                actor: sub,
                topic: "*".into(),
            },
            None,
        ).await;
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<TestProbe> for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

#[async_trait]
impl Receive<DeadLetter> for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: DeadLetter, _sender: Sender) {
        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn channel_dead_letters() {
    let sys = block_on(ActorSystem::new()).unwrap();
    let actor = block_on(sys
        .actor_of(DeadLetterSub::props(), "dl-subscriber"))
        .unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    let props = Props::new(DumbActor::new);
    let dumb = block_on(sys.actor_of(props, "dumb-actor")).unwrap();

    // immediately stop the actor and attempt to send a message
    block_on(sys.stop(&dumb));
    std::thread::sleep(std::time::Duration::from_secs(1));
    block_on(dumb.tell(SomeMessage, None));

    p_assert_eq!(listen, ());
}
