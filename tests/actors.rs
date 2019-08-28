#[macro_use]
extern crate riker_testkit;

use async_trait::async_trait;
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

#[async_trait]
impl Actor for Counter {
    // we used the #[actor] attribute so CounterMsg is the Msg type
    type Msg = CounterMsg;

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<TestProbe> for Counter {
    type Msg = CounterMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        self.probe = Some(msg)
    }
}

#[async_trait]
impl Receive<Add> for Counter {
    type Msg = CounterMsg;

    async fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Add, _sender: Sender) {
        self.count += 1;
        if self.count == 1_000_000 {
            self.probe.as_mut().unwrap().0.event(()).await;
        }
    }
}

#[test]
fn actor_create() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(Counter::actor);
        assert!(sys.actor_of(props.clone(), "valid-name").await.is_ok());

        assert!(sys.actor_of(props.clone(), "/").await.is_err());
        assert!(sys.actor_of(props.clone(), "*").await.is_err());
        assert!(sys.actor_of(props.clone(), "/a/b/c").await.is_err());
        assert!(sys.actor_of(props.clone(), "@").await.is_err());
        assert!(sys.actor_of(props.clone(), "#").await.is_err());
        assert!(sys.actor_of(props.clone(), "abc*").await.is_err());
        assert!(sys.actor_of(props, "!").await.is_err());
    })
}

#[test]
fn actor_tell() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(Counter::actor);
        let actor = sys.actor_of(props, "me").await.unwrap();

        let (probe, mut listen) = probe();
        actor.tell(TestProbe(probe), None).await;

        for _ in 0..1_000_000 {
            actor.tell(Add, None).await;
        }

        p_assert_eq!(listen, ());
    });
}

#[test]
fn actor_try_tell() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(Counter::actor);
        let actor = sys.actor_of(props, "me").await.unwrap();
        let actor: BasicActorRef = actor.into();

        let (probe, mut listen) = probe();
        actor.try_tell(CounterMsg::TestProbe(TestProbe(probe)), None).await.unwrap();

        assert!(actor.try_tell(CounterMsg::Add(Add), None).await.is_ok());
        assert!(actor.try_tell("invalid-type".to_string(), None).await.is_err());

        for _ in 0..1_000_000 {
            actor.try_tell(CounterMsg::Add(Add), None).await.unwrap();
        }

        p_assert_eq!(listen, ());
    });
}

struct Parent {
    probe: Option<TestProbe>,
}

impl Parent {
    fn actor() -> Self {
        Parent { probe: None }
    }
}

#[async_trait]
impl Actor for Parent {
    type Msg = TestProbe;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_a").await.unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_b").await.unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_c").await.unwrap();

        let props = Props::new(Child::actor);
        ctx.actor_of(props, "child_d").await.unwrap();
    }

    async fn post_stop(&mut self) {
        // All children have been terminated at this point
        // and we can signal back that the parent has stopped
        self.probe.as_mut().unwrap().0.event(()).await;
    }

    async fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        self.probe = Some(msg);
        self.probe.as_mut().unwrap().0.event(()).await;
    }
}

struct Child;

impl Child {
    fn actor() -> Self {
        Child
    }
}

#[async_trait]
impl Actor for Child {
    type Msg = ();

    async fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) { /* empty */ }
}

#[test]
#[allow(dead_code)]
fn actor_stop() {
    block_on(async {
        let system = ActorSystem::new().await.unwrap();

        let props = Props::new(Parent::actor);
        let parent = system.actor_of(props, "parent").await.unwrap();

        let (probe, mut listen) = probe();
        parent.tell(TestProbe(probe), None).await;
        system.print_tree();

        // wait for the probe to arrive at the actor before attempting to stop the actor
        listen.recv().await;

        system.stop(&parent).await;
        p_assert_eq!(listen, ());
    });
}
