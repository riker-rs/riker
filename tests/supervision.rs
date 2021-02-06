use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::p_assert_eq;

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

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
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

#[riker_testkit::test]
fn supervision_restart_failed_actor() {
    let sys = ActorSystem::new().unwrap();

    for i in 0..100 {
        let sup = sys
            .actor_of::<RestartSup>(&format!("supervisor_{}", i))
            .unwrap();

        // Make the test actor panic
        sup.tell(Panic, None);

        let (probe, mut listen) = probe::<()>();

        sup.tell(TestProbe(probe), None);
        p_assert_eq!(listen, ());
    }
}


// Test Escalate Strategy
#[actor(TestProbe, Panic)]
#[derive(Default)]
struct EscalateSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl Actor for EscalateSup {
    type Msg = EscalateSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.actor_to_fail = ctx.actor_of::<PanicActor>("actor-to-fail").ok();
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Escalate
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        // match msg {
        //     // We just resend the messages to the actor that we're concerned about testing
        //     TestMsg::Panic => self.actor_to_fail.try_tell(msg, None).unwrap(),
        //     TestMsg::Probe(_) => self.actor_to_fail.try_tell(msg, None).unwrap(),
        // };
    }
}

impl Receive<TestProbe> for EscalateSup {
    type Msg = EscalateSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(msg, sender);
    }
}

impl Receive<Panic> for EscalateSup {
    type Msg = EscalateSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None);
    }
}

#[actor(TestProbe, Panic)]
#[derive(Default)]
struct EscRestartSup {
    escalator: Option<ActorRef<EscalateSupMsg>>,
}

impl Actor for EscRestartSup {
    type Msg = EscRestartSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.escalator = ctx.actor_of::<EscalateSup>("escalate-supervisor").ok();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        // match msg {
        //     // We resend the messages to the parent of the actor that is/has panicked
        //     TestMsg::Panic => self.escalator.try_tell(msg, None).unwrap(),
        //     TestMsg::Probe(_) => self.escalator.try_tell(msg, None).unwrap(),
        // };
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }
}

impl Receive<TestProbe> for EscRestartSup {
    type Msg = EscRestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.escalator.as_ref().unwrap().tell(msg, sender);
    }
}

impl Receive<Panic> for EscRestartSup {
    type Msg = EscRestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.escalator.as_ref().unwrap().tell(Panic, None);
    }
}

#[riker_testkit::test]
fn supervision_escalate_failed_actor() {
    let sys = ActorSystem::new().unwrap();

    let sup = sys.actor_of::<EscRestartSup>("supervisor").unwrap();

    // Make the test actor panic
    sup.tell(Panic, None);

    let (probe, mut listen) = probe::<()>();

    #[cfg(feature = "tokio_executor")]
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    #[cfg(not(feature = "tokio_executor"))]
    std::thread::sleep(std::time::Duration::from_millis(2000));

    sup.tell(TestProbe(probe), None);
    p_assert_eq!(listen, ());
    sys.print_tree();
}
