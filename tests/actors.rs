#[macro_use]
extern crate riker_testkit;

use futures::executor::block_on;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

#[derive(Clone, Debug)]
pub struct Add;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[actor(TestProbe, Add)]
struct Counter {
    probe: Option<TestProbe>,
    count: u32,
}

impl Counter {
    fn actor() -> Counter {
        Counter {
            probe: None,
            count: 0,
        }
    }
}

impl Actor for Counter {
    // we used the #[actor] attribute so CounterMsg is the Msg type
    type Msg = CounterMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for Counter {
    type Msg = CounterMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        self.probe = Some(msg)
    }
}

impl Receive<Add> for Counter {
    type Msg = CounterMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Add, _sender: Sender) {
        self.count += 1;
        if self.count == 1_000_000 {
            self.probe.as_ref().unwrap().0.event(())
        }
    }
}

#[test]
fn actor_create() {
    let sys = block_on(ActorSystem::new()).unwrap();

    let props = Props::new(Counter::actor);
    assert!(block_on(sys.actor_of(props.clone(), "valid-name")).is_ok());

    assert!(block_on(sys.actor_of(props.clone(), "/")).is_err());
    assert!(block_on(sys.actor_of(props.clone(), "*")).is_err());
    assert!(block_on(sys.actor_of(props.clone(), "/a/b/c")).is_err());
    assert!(block_on(sys.actor_of(props.clone(), "@")).is_err());
    assert!(block_on(sys.actor_of(props.clone(), "#")).is_err());
    assert!(block_on(sys.actor_of(props.clone(), "abc*")).is_err());
    assert!(block_on(sys.actor_of(props, "!")).is_err());
}

#[test]
fn actor_tell() {
    let sys = block_on(ActorSystem::new()).unwrap();

    let props = Props::new(Counter::actor);
    let actor = block_on(sys.actor_of(props, "me")).unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    for _ in 0..1_000_000 {
        actor.tell(Add, None);
    }

    p_assert_eq!(listen, ());
}

#[test]
fn actor_try_tell() {
    let sys = block_on(ActorSystem::new()).unwrap();

    let props = Props::new(Counter::actor);
    let actor = block_on(sys.actor_of(props, "me")).unwrap();
    let actor: BasicActorRef = actor.into();

    let (probe, listen) = probe();
    block_on(actor.try_tell(CounterMsg::TestProbe(TestProbe(probe)), None)).unwrap();

    assert!(block_on(actor.try_tell(CounterMsg::Add(Add), None)).is_ok());
    assert!(block_on(actor.try_tell("invalid-type".to_string(), None)).is_err());

    for _ in 0..1_000_000 {
        block_on(actor.try_tell(CounterMsg::Add(Add), None)).unwrap();
    }

    p_assert_eq!(listen, ());
}

struct Parent {
    probe: Option<TestProbe>,
}

impl Parent {
    fn actor() -> Self {
        Parent { probe: None }
    }
}

impl Actor for Parent {
    type Msg = TestProbe;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_a").unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_b").unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_c").unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_d").unwrap();
    }

    fn post_stop(&mut self) {
        // All children have been terminated at this point
        // and we can signal back that the parent has stopped
        self.probe.as_ref().unwrap().0.event(());
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        self.probe = Some(msg);
        self.probe.as_ref().unwrap().0.event(());
    }
}

struct Child;

impl Child {
    fn actor() -> Self {
        Child
    }
}

impl Actor for Child {
    type Msg = ();

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
#[allow(dead_code)]
fn actor_stop() {
    let system = block_on(ActorSystem::new()).unwrap();

    let props = Props::new(Parent::actor);
    let parent = system.actor_of(props, "parent").unwrap();

    let (probe, listen) = probe();
    parent.tell(TestProbe(probe), None);
    system.print_tree();

    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    system.stop(&parent);
    p_assert_eq!(listen, ());
}
