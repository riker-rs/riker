#[macro_use]
extern crate riker_testkit;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

#[derive(Clone, Debug)]
pub struct Panic;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[derive(Default)]
struct DumbActor;

impl Actor for DumbActor {
    type Msg = ();

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[actor(TestProbe, Panic)]
#[derive(Default)]
struct PanicActor;

impl Actor for PanicActor {
    type Msg = PanicActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.actor_of::<DumbActor>("child_a").unwrap();

        ctx.actor_of::<DumbActor>("child_b").unwrap();

        ctx.actor_of::<DumbActor>("child_c").unwrap();

        ctx.actor_of::<DumbActor>("child_d").unwrap();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for PanicActor {
    type Msg = PanicActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        msg.0.event(());
    }
}

impl Receive<Panic> for PanicActor {
    type Msg = PanicActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

// Test Restart Strategy
#[actor(TestProbe, Panic)]
#[derive(Default)]
struct RestartSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl Actor for RestartSup {
    type Msg = RestartSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.actor_to_fail = ctx.actor_of::<PanicActor>("actor-to-fail").ok();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender)
    }
}

impl Receive<TestProbe> for RestartSup {
    type Msg = RestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(msg, sender);
    }
}

impl Receive<Panic> for RestartSup {
    type Msg = RestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn supervision_restart_failed_actor() {
    let backend = tokio::runtime::Handle::current().into();
    let sys = ActorSystem::new(backend).unwrap();

    for i in 0..100 {
        let sup = sys
            .actor_of::<RestartSup>(&format!("supervisor_{}", i))
            .unwrap();

        // Make the test actor panic
        sup.tell(Panic, None);

        let (probe, listen) = probe::<()>();
        sup.tell(TestProbe(probe), None);
        p_assert_eq!(listen, ());
    }
}
