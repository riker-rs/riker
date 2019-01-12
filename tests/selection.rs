#[macro_use]
extern crate riker_testkit;

use riker::actors::*;
use riker_default::DefaultModel;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

#[derive(Clone, Debug)]
struct TestMsg(TestProbe);
type TestProbe = ChannelProbe<(), ()>;

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

// a simple minimal actor for use in tests
struct ChildActor;

impl ChildActor {
    fn new() -> BoxActor<TestMsg> {
        Box::new(ChildActor)
    }
}

impl Actor for ChildActor {
    type Msg = TestMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        let probe = msg;
        probe.0.event(());
    }
}

#[derive(Clone)]
struct SelectTestActor;

impl SelectTestActor {
    fn new() -> BoxActor<TestMsg> {
        Box::new(SelectTestActor)
    }
}

impl Actor for SelectTestActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // create first child actor
        let props = Props::new(Box::new(ChildActor::new));
        let _ = ctx.actor_of(props, "child_a").unwrap();

        // create second child actor
        let props = Props::new(Box::new(ChildActor::new));
        let _ = ctx.actor_of(props, "child_b").unwrap();
    }

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        let probe = msg;
        probe.0.event(());
    }
}

#[test]
fn select_child() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(SelectTestActor::new));
    system.actor_of(props, "select-actor").unwrap();

    let (probe, listen) = probe();

    // select test actors through actor selection: /root/user/select-actor/*
    let selection = system.select("select-actor").unwrap();
    
    selection.tell(TestMsg(probe), None);

    p_assert_eq!(listen, ());
}

#[test]
fn select_child_of_child() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(SelectTestActor::new));
    system.actor_of(props, "select-actor").unwrap();

    // delay to allow 'select-actor' pre_start to create 'child_a' and 'child_b'
    // Direct messaging on the actor_ref doesn't have this same issue
    std::thread::sleep(std::time::Duration::from_millis(500));

    let (probe, listen) = probe();

    // select test actors through actor selection: /root/user/select-actor/*
    let selection = system.select("select-actor/child_a").unwrap();
    selection.tell(TestMsg(probe), None);

    // actors 'child_a' should fire a probe event
    p_assert_eq!(listen, ());
}

#[test]
fn select_all_children_of_child() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(SelectTestActor::new));
    system.actor_of(props, "select-actor").unwrap();

    // delay to allow 'select-actor' pre_start to create 'child_a' and 'child_b'
    // Direct messaging on the actor_ref doesn't have this same issue
    std::thread::sleep(std::time::Duration::from_millis(500));

    let (probe, listen) = probe();

    // select relative test actors through actor selection: /root/user/select-actor/*
    let selection = system.select("select-actor/*").unwrap();
    selection.tell(TestMsg(probe.clone()), None);

    // actors 'child_a' and 'child_b' should both fire a probe event
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());

    // select absolute test actors through actor selection: /root/user/select-actor/*
    let selection = system.select("/user/select-actor/*").unwrap();
    selection.tell(TestMsg(probe), None);

    // actors 'child_a' and 'child_b' should both fire a probe event
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}

#[derive(Clone)]
struct SelectTestActorCtx;

impl SelectTestActorCtx {
    fn new() -> BoxActor<TestMsg> {
        Box::new(SelectTestActorCtx)
    }
}

impl Actor for SelectTestActorCtx {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // create first child actor
        let props = Props::new(Box::new(ChildActor::new));
        let _ = ctx.actor_of(props, "child_a").unwrap();

        // create second child actor
        let props = Props::new(Box::new(ChildActor::new));
        let _ = ctx.actor_of(props, "child_b").unwrap();
    }

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {

        // up and down: ../select-actor/child_a
        let sel = ctx.select("../select-actor/child_a").unwrap();
        sel.tell(msg.clone(), None);

        // child: child_a
        let sel = ctx.select("child_a").unwrap();
        sel.tell(msg.clone(), None);

        // absolute: /user/select-actor/child_a
        let sel = ctx.select("/user/select-actor/child_a").unwrap();
        sel.tell(msg.clone(), None);

        // // absolute all: /user/select-actor/*
        let sel = ctx.select("/user/select-actor/*").unwrap();
        sel.tell(msg.clone(), None);

        // all: *
        let sel = ctx.select("*").unwrap();
        sel.tell(msg, None);
    }
}

#[test]
fn select_ctx() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(SelectTestActorCtx::new));
    let actor = system.actor_of(props, "select-actor").unwrap();

    let (probe, listen) = probe();
    actor.tell(TestMsg(probe), None);

    // seven test results expected:
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}


// *** Dead letters test ***
struct DeadLettersActor {
    probe: Option<TestProbe>,
}

impl DeadLettersActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = DeadLettersActor {
            probe: None
        };

        Box::new(actor)
    }
}

impl Actor for DeadLettersActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // subscribe to dead_letters
        let msg = ChannelMsg::Subscribe(All.into(), ctx.myself());
        ctx.system.dead_letters().tell(msg, None);
    }

    fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        msg.0.event(()); // notify listen then probe has been received.
        self.probe = Some(msg.0);
    }

    fn other_receive(&mut self, _: &Context<Self::Msg>, msg: ActorMsg<Self::Msg>, _: Option<ActorRef<Self::Msg>>) {
        if let ActorMsg::DeadLetter(dl) = msg {
            println!("DeadLetter: {} => {} ({:?})", dl.sender, dl.recipient, dl.msg);
            self.probe.event(());
        }
    }
}

#[test]
fn select_no_actors() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let sys = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(DeadLettersActor::new));
    let act = sys.actor_of(props, "dl-subscriber").unwrap();

    let (probe, listen) = probe();
    act.tell(TestMsg(probe.clone()), None);
    
    // wait for the probe to arrive at the dl sub before doing select
    listen.recv();

    let sel = sys.select("nothing-here").unwrap();
    
    sel.tell(TestMsg(probe), None);

    p_assert_eq!(listen, ());
}

#[test]
fn select_invalid_path() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let sys = ActorSystem::new(&model).unwrap();

    // todo randomize the invalid character

    let sel = sys.select("foo/`");
    assert!(sel.is_err());
    let sel = sys.select("foo/@");
    assert!(sel.is_err());
    let sel = sys.select("!");
    assert!(sel.is_err());
    let sel = sys.select("foo/$");
    assert!(sel.is_err());
    let sel = sys.select("&");
    assert!(sel.is_err());
}