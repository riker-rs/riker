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
    Panic,
}

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}


// a simple minimal actor for use in tests
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

struct PanicActor;

impl PanicActor {
    fn new() -> BoxActor<TestMsg> {
        Box::new(PanicActor)
    }
}

impl Actor for PanicActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Box::new(DumbActor::new));
        ctx.actor_of(props, "child_a").unwrap();

        let props = Props::new(Box::new(DumbActor::new));
        ctx.actor_of(props, "child_b").unwrap();

        let props = Props::new(Box::new(DumbActor::new));
        ctx.actor_of(props, "child_c").unwrap();

        let props = Props::new(Box::new(DumbActor::new));
        ctx.actor_of(props, "child_d").unwrap();
    }

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Panic => panic!("// TEST PANIC // TEST PANIC // TEST PANIC //"),
            TestMsg::Probe(probe) => {
                probe.event(())
            }
        }
    }
}

// Test Restart Strategy
struct RestartSupervisor {
    actor_to_fail: Option<ActorRef<TestMsg>>,
}

impl RestartSupervisor {
    fn new() -> BoxActor<TestMsg> {
        let actor = RestartSupervisor {
            actor_to_fail: None
        };
        Box::new(actor)
    }
}

impl Actor for RestartSupervisor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Box::new(PanicActor::new));
        self.actor_to_fail = ctx.actor_of(props, "actor-to-fail").ok();
    }

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<ActorRef<Self::Msg>>) {
        match msg {
            // We resend the messages to the actor that is/has panicked
            TestMsg::Panic => self.actor_to_fail.try_tell(msg, None).unwrap(),
            TestMsg::Probe(_) => self.actor_to_fail.try_tell(msg, None).unwrap(),
        };
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }
}

#[test]
fn supervision_restart_failed_actor() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    for i in 0..10 {
        let props = Props::new(Box::new(RestartSupervisor::new));
        let sup = system.actor_of(props, &format!("supervisor_{}", i)).unwrap();

        // Make the test actor panic
        sup.tell(TestMsg::Panic, None);

        let (probe, listen) = probe::<()>();
        sup.tell(TestMsg::Probe(probe), None);
        p_assert_eq!(listen, ());
    }
}

// Test Escalate Strategy
struct EscalateSupervisor {
    actor_to_fail: Option<ActorRef<TestMsg>>,
}

impl EscalateSupervisor {
    fn new() -> BoxActor<TestMsg> {
        let actor = EscalateSupervisor {
            actor_to_fail: None
        };
        Box::new(actor)
    }
}

impl Actor for EscalateSupervisor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Box::new(PanicActor::new));
        self.actor_to_fail = ctx.actor_of(props, "actor-to-fail").ok();
    }

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<ActorRef<Self::Msg>>) {
        match msg {
            // We just resend the messages to the actor that we're concerned about testing
            TestMsg::Panic => self.actor_to_fail.try_tell(msg, None).unwrap(),
            TestMsg::Probe(_) => self.actor_to_fail.try_tell(msg, None).unwrap(),
        };
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Escalate
    }
}

struct EscRestartSupervisor {
    escalator: Option<ActorRef<TestMsg>>,
}

impl EscRestartSupervisor {
    fn new() -> BoxActor<TestMsg> {
        let actor = EscRestartSupervisor {
            escalator: None
        };
        Box::new(actor)
    }
}

impl Actor for EscRestartSupervisor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Box::new(EscalateSupervisor::new));
        self.escalator = ctx.actor_of(props, "escalate-supervisor").ok();
    }

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<ActorRef<Self::Msg>>) {
        match msg {
            // We resend the messages to the parent of the actor that is/has panicked
            TestMsg::Panic => self.escalator.try_tell(msg, None).unwrap(),
            TestMsg::Probe(_) => self.escalator.try_tell(msg, None).unwrap(),
        };
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }
}

#[test]
// removed for now until race condition fixed
fn supervision_escalate_failed_actor() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(EscRestartSupervisor::new));
    let sup = system.actor_of(props, "supervisor").unwrap();

    // Make the test actor panic
    sup.tell(TestMsg::Panic, None);

    let (probe, listen) = probe::<()>();
    std::thread::sleep(std::time::Duration::from_millis(2000));
    sup.tell(TestMsg::Probe(probe), None);
    p_assert_eq!(listen, ());
    system.print_tree();
}