#[macro_use]
extern crate riker_testkit;

use async_trait::async_trait;
use futures::executor::block_on;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

#[derive(Clone, Debug)]
pub struct Panic;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

struct DumbActor;

impl DumbActor {
    fn new() -> Self {
        DumbActor
    }
}

#[async_trait]
impl Actor for DumbActor {
    type Msg = ();

    async fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[actor(TestProbe, Panic)]
struct PanicActor;

impl PanicActor {
    fn new() -> Self {
        PanicActor
    }
}

#[async_trait]
impl Actor for PanicActor {
    type Msg = PanicActorMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(DumbActor::new);
        ctx.actor_of(props, "child_a").await.unwrap();

        let props = Props::new(DumbActor::new);
        ctx.actor_of(props, "child_b").await.unwrap();

        let props = Props::new(DumbActor::new);
        ctx.actor_of(props, "child_c").await.unwrap();

        let props = Props::new(DumbActor::new);
        ctx.actor_of(props, "child_d").await.unwrap();
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<TestProbe> for PanicActor {
    type Msg = PanicActorMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, mut msg: TestProbe, _sender: Sender) {
        msg.0.event(()).await;
    }
}

#[async_trait]
impl Receive<Panic> for PanicActor {
    type Msg = PanicActorMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

// Test Restart Strategy
#[actor(TestProbe, Panic)]
struct RestartSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl RestartSup {
    fn new() -> Self {
        RestartSup {
            actor_to_fail: None,
        }
    }
}

#[async_trait]
impl Actor for RestartSup {
    type Msg = RestartSupMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(PanicActor::new);
        self.actor_to_fail = ctx.actor_of(props, "actor-to-fail").await.ok();
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }
}

#[async_trait]
impl Receive<TestProbe> for RestartSup {
    type Msg = RestartSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(msg, sender);
    }
}

#[async_trait]
impl Receive<Panic> for RestartSup {
    type Msg = RestartSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None).await;
    }
}

#[test]
fn supervision_restart_failed_actor() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        for i in 0..100 {
            let props = Props::new(RestartSup::new);
            let name = format!("supervisor_{}", i);
            let sup = sys.actor_of(props, &name).await.unwrap();

            // Make the test actor panic
            sup.tell(Panic, None).await;

            let (probe, mut listen) = probe::<()>();
            sup.tell(TestProbe(probe), None).await;
            p_assert_eq!(listen, ());
        }
    });
}

// Test Escalate Strategy
#[actor(TestProbe, Panic)]
struct EscalateSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl EscalateSup {
    fn new() -> Self {
        EscalateSup {
            actor_to_fail: None,
        }
    }
}

#[async_trait]
impl Actor for EscalateSup {
    type Msg = EscalateSupMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(PanicActor::new);
        self.actor_to_fail = ctx.actor_of(props, "actor-to-fail").await.ok();
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
        // match msg {
        //     // We just resend the messages to the actor that we're concerned about testing
        //     TestMsg::Panic => self.actor_to_fail.try_tell(msg, None).unwrap(),
        //     TestMsg::Probe(_) => self.actor_to_fail.try_tell(msg, None).unwrap(),
        // };
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Escalate
    }
}

#[async_trait]
impl Receive<TestProbe> for EscalateSup {
    type Msg = EscalateSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(msg, sender).await;
    }
}

#[async_trait]
impl Receive<Panic> for EscalateSup {
    type Msg = EscalateSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None).await;
    }
}

#[actor(TestProbe, Panic)]
struct EscRestartSup {
    escalator: Option<ActorRef<EscalateSupMsg>>,
}

impl EscRestartSup {
    fn new() -> Self {
        EscRestartSup { escalator: None }
    }
}

#[async_trait]
impl Actor for EscRestartSup {
    type Msg = EscRestartSupMsg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(EscalateSup::new);
        self.escalator = ctx.actor_of(props, "escalate-supervisor").await.ok();
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
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

#[async_trait]
impl Receive<TestProbe> for EscRestartSup {
    type Msg = EscRestartSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, sender: Sender) {
        self.escalator.as_ref().unwrap().tell(msg, sender).await;
    }
}

#[async_trait]
impl Receive<Panic> for EscRestartSup {
    type Msg = EscRestartSupMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.escalator.as_ref().unwrap().tell(Panic, None).await;
    }
}

#[test]
fn supervision_escalate_failed_actor() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(EscRestartSup::new);
        let sup = sys.actor_of(props, "supervisor").await.unwrap();

        // Make the test actor panic
        sup.tell(Panic, None).await;

        let (probe, mut listen) = probe::<()>();
        std::thread::sleep(std::time::Duration::from_millis(2000));
        sup.tell(TestProbe(probe), None);
        p_assert_eq!(listen, ());
        sys.print_tree();
    });
}
