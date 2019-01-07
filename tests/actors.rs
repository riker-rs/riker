#[macro_use]
extern crate riker_testkit;

use riker::actors::*;
use riker_default::DefaultModel;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

#[derive(Clone, Debug)]
enum TestMsg {
    Probe(TestProbe),
    Add,
}
type TestProbe = ChannelProbe<(), ()>;

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

#[derive(Clone)]
struct TellActor {
    probe: Option<TestProbe>,
    count: u32,
}

impl TellActor {
    fn actor() -> BoxActor<TestMsg> {
        let a = TellActor {
            probe: None,
            count: 0
        };

        Box::new(a)
    }
}

impl Actor for TellActor {
    type Msg = TestMsg;

    fn receive(&mut self,
                _ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                _sender: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Probe(p) => self.probe = Some(p),
            TestMsg::Add => {
                self.count += 1;
                if self.count == 1_000_000 {
                    self.probe.event(())
                }
            }
        }
    }
}

#[test]
fn tell_actor() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(TellActor::actor));
    let actor = system.actor_of(props, "me").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestMsg::Probe(probe), None);

    for _ in 0..1_000_000 {
        actor.tell(TestMsg::Add, None);
    }

    p_assert_eq!(listen, ());
}

struct ParentActor {
    probe: Option<TestProbe>,
}

impl ParentActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = ParentActor {
            probe: None
        };

        Box::new(actor)
    }
}

impl Actor for ParentActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let props = Props::new(Box::new(ChildActor::new));
        ctx.actor_of(props, "child_a").unwrap();

        let props = Props::new(Box::new(ChildActor::new));
        ctx.actor_of(props, "child_b").unwrap();

        let props = Props::new(Box::new(ChildActor::new));
        ctx.actor_of(props, "child_c").unwrap();

        let props = Props::new(Box::new(ChildActor::new));
        ctx.actor_of(props, "child_d").unwrap();
    }

    fn post_stop(&mut self) {
        // All children have been terminated at this point
        // and we can signal back that the parent has stopped
        self.probe.as_ref().unwrap().event(());
    }

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<ActorRef<Self::Msg>>) {
        if let TestMsg::Probe(p) = msg {
            self.probe = Some(p);
            self.probe.event(());
        }
    }
}

struct ChildActor;

impl ChildActor {
    fn new() -> BoxActor<TestMsg> {
        Box::new(ChildActor)
    }
}

impl Actor for ChildActor {
    type Msg = TestMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
}

#[test]
#[allow(dead_code)]
fn stop_actor() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(ParentActor::new));
    let parent = system.actor_of(props, "parent").unwrap();

    std::thread::sleep(std::time::Duration::from_secs(2));

    let (probe, listen) = probe();
    parent.tell(TestMsg::Probe(probe), None);
    system.print_tree();

    // wait for the probe to arrive at the actor before attempting to stop the actor
    listen.recv();
    
    system.stop(&parent);
    p_assert_eq!(listen, ());
}

