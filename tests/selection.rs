#[macro_use]
extern crate riker_testkit;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

// a simple minimal actor for use in tests
// #[actor(TestProbe)]
#[derive(Default)]
struct Child;

impl Actor for Child {
    type Msg = TestProbe;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        msg.0.event(());
    }
}

#[derive(Default)]
struct SelectTest;

impl Actor for SelectTest {
    type Msg = TestProbe;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // create first child actor
        let _ = ctx.actor_of::<Child>("child_a").unwrap();

        // create second child actor
        let _ = ctx.actor_of::<Child>("child_b").unwrap();
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        msg.0.event(());
    }
}

#[tokio::test]
async fn select_child() {
    let sys = ActorSystem::new().unwrap();

    sys.actor_of::<SelectTest>("select-actor").unwrap();

    let (probe, mut listen) = probe();

    // select test actors through actor selection: /root/user/select-actor/*
    let sel = sys.select("select-actor").unwrap();

    sel.try_tell(TestProbe(probe), None);

    p_assert_eq!(listen, ());
}

#[tokio::test]
async fn select_child_of_child() {
    let sys = ActorSystem::new().unwrap();

    sys.actor_of::<SelectTest>("select-actor").unwrap();

    // delay to allow 'select-actor' pre_start to create 'child_a' and 'child_b'
    // Direct messaging on the actor_ref doesn't have this same issue
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let (probe, mut listen) = probe();

    // select test actors through actor selection: /root/user/select-actor/*
    let sel = sys.select("select-actor/child_a").unwrap();
    sel.try_tell(TestProbe(probe), None);

    // actors 'child_a' should fire a probe event
    p_assert_eq!(listen, ());
}

#[tokio::test]
async fn select_all_children_of_child() {
    let sys = ActorSystem::new().unwrap();

    sys.actor_of::<SelectTest>("select-actor").unwrap();

    // delay to allow 'select-actor' pre_start to create 'child_a' and 'child_b'
    // Direct messaging on the actor_ref doesn't have this same issue
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let (probe, mut listen) = probe();

    // select relative test actors through actor selection: /root/user/select-actor/*
    let sel = sys.select("select-actor/*").unwrap();
    sel.try_tell(TestProbe(probe.clone()), None);

    // actors 'child_a' and 'child_b' should both fire a probe event
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());

    // select absolute test actors through actor selection: /root/user/select-actor/*
    let sel = sys.select("/user/select-actor/*").unwrap();
    sel.try_tell(TestProbe(probe), None);

    // actors 'child_a' and 'child_b' should both fire a probe event
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}

#[derive(Clone, Default)]
struct SelectTest2;

impl Actor for SelectTest2 {
    type Msg = TestProbe;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // create first child actor
        let _ = ctx.actor_of::<Child>("child_a").unwrap();

        // create second child actor
        let _ = ctx.actor_of::<Child>("child_b").unwrap();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        // up and down: ../select-actor/child_a
        let sel = ctx.select("../select-actor/child_a").unwrap();
        sel.try_tell(msg.clone(), None);

        // child: child_a
        let sel = ctx.select("child_a").unwrap();
        sel.try_tell(msg.clone(), None);

        // absolute: /user/select-actor/child_a
        let sel = ctx.select("/user/select-actor/child_a").unwrap();
        sel.try_tell(msg.clone(), None);

        // absolute all: /user/select-actor/*
        let sel = ctx.select("/user/select-actor/*").unwrap();
        sel.try_tell(msg.clone(), None);

        // all: *
        let sel = ctx.select("*").unwrap();
        sel.try_tell(msg, None);
    }
}

#[tokio::test]
async fn select_from_context() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<SelectTest2>("select-actor").unwrap();

    let (probe, mut listen) = probe();

    actor.tell(TestProbe(probe), None);

    // seven events back expected:
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
    p_assert_eq!(listen, ());
}

#[tokio::test]
async fn select_paths() {
    let sys = ActorSystem::new().unwrap();

    assert!(sys.select("foo/").is_ok());
    assert!(sys.select("/foo/").is_ok());
    assert!(sys.select("/foo").is_ok());
    assert!(sys.select("/foo/..").is_ok());
    assert!(sys.select("../foo/").is_ok());
    assert!(sys.select("/foo/*").is_ok());
    assert!(sys.select("*").is_ok());

    assert!(sys.select("foo/`").is_err());
    assert!(sys.select("foo/@").is_err());
    assert!(sys.select("!").is_err());
    assert!(sys.select("foo/$").is_err());
    assert!(sys.select("&").is_err());
}

// // *** Dead letters test ***
// #[derive(Default)]
// struct DeadLettersActor {
//     probe: Option<TestProbe>,
// }

// impl Actor for DeadLettersActor {
//     type Msg = TestMsg;

//     fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
//         // subscribe to dead_letters
//         let msg = ChannelMsg::Subscribe(All.into(), ctx.myself());
//         ctx.system.dead_letters().tell(msg, None);
//     }

//     fn receive(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
//         msg.0.event(()); // notify listen then probe has been received.
//         self.probe = Some(msg.0);
//     }

//     fn other_receive(&mut self, _: &Context<Self::Msg>, msg: ActorMsg<Self::Msg>, _: Option<ActorRef<Self::Msg>>) {
//         if let ActorMsg::DeadLetter(dl) = msg {
//             println!("DeadLetter: {} => {} ({:?})", dl.sender, dl.recipient, dl.msg);
//             self.probe.event(());
//         }
//     }
// }

// #[test]
// fn select_no_actors() {
//     let sys = ActorSystem::new().unwrap();

//     let act = sys.actor_of::<DeadLettersActor>("dl-subscriber").unwrap();

//     let (probe, listen) = probe();
//     act.tell(TestMsg(probe.clone()), None);

//     // wait for the probe to arrive at the dl sub before doing select
//     listen.recv();

//     let sel = sys.select("nothing-here").unwrap();

//     sel.tell(TestMsg(probe), None);

//     p_assert_eq!(listen, ());
// }
