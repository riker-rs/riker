#[macro_use]
extern crate riker_testkit;

use tezedge_actor_system::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

#[derive(Clone, Debug)]
pub struct Add;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[actor(TestProbe, Add)]
#[derive(Default)]
struct Counter {
    probe: Option<TestProbe>,
    count: u32,
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
    let sys = ActorSystem::new().unwrap();

    assert!(sys.actor_of::<Counter>("valid-name").is_ok());

    match sys.actor_of::<Counter>("/") {
        Ok(_) => panic!("test should not reach here"),
        Err(e) => {
            // test Display
            assert_eq!(
                e.to_string(),
                "Failed to create actor. Cause: Invalid actor name (/)"
            );
            assert_eq!(
                format!("{}", e),
                "Failed to create actor. Cause: Invalid actor name (/)"
            );
            // test Debug
            assert_eq!(format!("{:?}", e), "InvalidName(\"/\")");
            assert_eq!(format!("{:#?}", e), "InvalidName(\n    \"/\",\n)");
        }
    }
    assert!(sys.actor_of::<Counter>("*").is_err());
    assert!(sys.actor_of::<Counter>("/a/b/c").is_err());
    assert!(sys.actor_of::<Counter>("@").is_err());
    assert!(sys.actor_of::<Counter>("#").is_err());
    assert!(sys.actor_of::<Counter>("abc*").is_err());
    assert!(sys.actor_of::<Counter>("!").is_err());
}

#[test]
fn actor_tell() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<Counter>("me").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestProbe(probe), None);

    for _ in 0..1_000_000 {
        actor.tell(Add, None);
    }

    p_assert_eq!(listen, ());
}

#[test]
fn actor_try_tell() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<Counter>("me").unwrap();
    let actor: BasicActorRef = actor.into();

    let (probe, listen) = probe();
    actor
        .try_tell(CounterMsg::TestProbe(TestProbe(probe)), None)
        .unwrap();

    assert!(actor.try_tell(CounterMsg::Add(Add), None).is_ok());
    assert!(actor.try_tell("invalid-type".to_string(), None).is_err());

    for _ in 0..1_000_000 {
        actor.try_tell(CounterMsg::Add(Add), None).unwrap();
    }

    p_assert_eq!(listen, ());
}

#[derive(Default)]
struct Parent {
    probe: Option<TestProbe>,
}

impl Actor for Parent {
    type Msg = TestProbe;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.actor_of::<Child>("child_a").unwrap();

        ctx.actor_of::<Child>("child_b").unwrap();

        ctx.actor_of::<Child>("child_c").unwrap();

        ctx.actor_of::<Child>("child_d").unwrap();
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

#[derive(Default)]
struct Child;

impl Actor for Child {
    type Msg = ();

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
fn actor_stop() {
    let system = ActorSystem::new().unwrap();

    let parent = system.actor_of::<Parent>("parent").unwrap();

    let (probe, listen) = probe();
    parent.tell(TestProbe(probe), None);
    system.print_tree();

    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();

    system.stop(&parent);
    p_assert_eq!(listen, ());
}
