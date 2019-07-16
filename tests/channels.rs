#[macro_use]
extern crate riker_testkit;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{ChannelProbe, probe};

use riker::actors::*;

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
        Props::new_args(Box::new(Subscriber::actor), (chan, topic))
    }
}

impl Actor for Subscriber {
    type Msg = SubscriberMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Box::new(ctx.myself());
        self.chan.tell(Subscribe { actor: sub, topic: self.topic.clone() }, None);
    }

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for Subscriber {
    type Msg = SubscriberMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               msg: TestProbe,
               _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

impl Receive<SomeMessage> for Subscriber {
    type Msg = SubscriberMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               _msg: SomeMessage,
               _sender: Sender) {
        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn channel_publish() {
    let sys = ActorSystem::new().unwrap();

    // Create the channel we'll be using
    let chan: ChannelRef<SomeMessage> = channel("my-chan", &sys).unwrap();

    // The topic we'll be publishing to. Endow our subscriber test actor with this.
    // On Subscriber's pre_start it will subscribe to this channel+topic
    let topic = Topic::from("my-topic");
    let sub = sys.actor_of_props(Subscriber::props(chan.clone(), topic.clone()), "sub-actor").unwrap();

    let (probe, listen) = probe();
    sub.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before publishing message
    listen.recv();

    // Publish a test message
    chan.tell(Publish { msg: SomeMessage, topic: topic }, None);

    p_assert_eq!(listen, ());
}

#[test]
fn channel_publish_subscribe_all() {
    let sys = ActorSystem::new().unwrap();

    // Create the channel we'll be using
    let chan: ChannelRef<SomeMessage> = channel("my-chan", &sys).unwrap();

    // The '*' All topic. Endow our subscriber test actor with this.
    // On Subscriber's pre_start it will subscribe to all topics on this channel.
    let topic = Topic::from("*");
    let sub = sys.actor_of_props(Subscriber::props(chan.clone(), topic.clone()), "sub-actor").unwrap();

    let (probe, listen) = probe();
    sub.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before publishing message
    listen.recv();

    // Publish a test message to topic "topic-1"
    chan.tell(Publish { msg: SomeMessage, topic: "topic-1".into() }, None);

    // Publish a test message to topic "topic-2"
    chan.tell(Publish { msg: SomeMessage, topic: "topic-2".into() }, None);

    // Publish a test message to topic "topic-3"
    chan.tell(Publish { msg: SomeMessage, topic: "topic-3".into() }, None);

    // Expecting three probe events
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}

#[derive(Clone, Debug)]
pub struct Panic;

#[actor(Panic, SomeMessage)]
#[derive(Default)]
struct DumbActor;

impl Actor for DumbActor {
    type Msg = DumbActorMsg;

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Panic> for DumbActor {
    type Msg = DumbActorMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               _msg: Panic,
               _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

impl Receive<SomeMessage> for DumbActor {
    type Msg = DumbActorMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               _msg: SomeMessage,
               _sender: Sender) {

        // Intentionally left blank
    }
}

// We must wrap SystemEvent in a type defined in this test crate
// so we can implement traits on it
#[derive(Clone, Debug)]
struct SysEvent(SystemEvent);

// *** Event stream test ***
#[actor(TestProbe, SystemEvent)]
#[derive(Default)]
struct EventSubscriber {
    probe: Option<TestProbe>,
}

impl Actor for EventSubscriber {
    type Msg = EventSubscriberMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe
        let sub = Box::new(ctx.myself());
        ctx.system
            .sys_events()
            .tell(Subscribe { actor: sub, topic: "*".into() }, None);
    }

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {
        self.receive(ctx, msg, sender);
    }

    fn sys_recv(&mut self,
                ctx: &Context<Self::Msg>,
                msg: SystemMsg,
                sender: Sender) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender);
        }
    }
}

impl Receive<TestProbe> for EventSubscriber {
    type Msg = EventSubscriberMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               msg: TestProbe,
               _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

impl Receive<SystemEvent> for EventSubscriber {
    type Msg = EventSubscriberMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               msg: SystemEvent,
               _sender: Sender) {
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
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<EventSubscriber>("event-sub").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before attempting
    // create, restart and stop
    listen.recv();

    // Create an actor
    let dumb = sys.actor_of::<DumbActor>("dumb-actor").unwrap();
    // ActorCreated event was received
    p_assert_eq!(listen, ());

    // Force restart of actor
    dumb.tell(Panic, None);
    // ActorRestarted event was received
    p_assert_eq!(listen, ());

    // Terminate actor
    sys.stop(&dumb);
    // ActorTerminated event was receive
    p_assert_eq!(listen, ());
}


// *** Dead letters test ***
#[actor(TestProbe, DeadLetter)]
#[derive(Default)]
struct DeadLetterSub {
    probe: Option<TestProbe>,
}

impl Actor for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe to dead_letters
        let sub = Box::new(ctx.myself());
        ctx.system
            .dead_letters()
            .tell(Subscribe { actor: sub, topic: "*".into() }, None);
    }

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {
        self.receive(ctx, msg, sender)
    }
}

impl Receive<TestProbe> for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               msg: TestProbe,
               _sender: Sender) {
        msg.0.event(());
        self.probe = Some(msg);
    }
}

impl Receive<DeadLetter> for DeadLetterSub {
    type Msg = DeadLetterSubMsg;

    fn receive(&mut self,
               _ctx: &Context<Self::Msg>,
               _msg: DeadLetter,
               _sender: Sender) {
        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn channel_dead_letters() {
    let sys = ActorSystem::new().unwrap();
    let actor = sys.actor_of::<DeadLetterSub>("dl-subscriber").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    let dumb = sys.actor_of::<DumbActor>("dumb-actor").unwrap();

    // immediately stop the actor and attempt to send a message
    sys.stop(&dumb);
    std::thread::sleep(std::time::Duration::from_secs(1));
    dumb.tell(SomeMessage, None);

    p_assert_eq!(listen, ());
}
