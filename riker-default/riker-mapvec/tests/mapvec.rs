extern crate riker;
extern crate riker_default;
#[macro_use]
extern crate riker_testkit;

use riker::actors::*;
use riker_default::DefaultModel;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

#[derive(Clone, Debug)]
struct TestProbe(ChannelProbe<(), u32>);

#[derive(Clone, Debug)]
enum TestMsg {
    Probe(TestProbe),
    Data(u32),
    Panic,
    ReportState,
}

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

struct PersistActor {
    id: String,
    state: u32,
    probe: Option<TestProbe>,
}

impl PersistActor {
    fn new(id: String) -> BoxActor<TestMsg> {
        let actor = PersistActor {
            id: id,
            state: 0,
            probe: None,
        };
        Box::new(actor)
    }

    fn update_state(&mut self, value: u32) {
        self.state += value;
    }
}

impl Actor for PersistActor {
    type Msg = TestMsg;

    fn receive(&mut self,
                ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                _sender: Option<ActorRef<Self::Msg>>) {

        match msg {
            TestMsg::Data(s) => {
                ctx.persist_event(TestMsg::Data(s));
            }
            TestMsg::Probe(ref probe) => {
                self.probe = Some(probe.clone());
            }
            TestMsg::ReportState => {
                self.probe.as_ref().unwrap().0.event(self.state);
            }
            TestMsg::Panic => {
                panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
            }
        }
    }

    fn apply_event(&mut self, _ctx: &Context<Self::Msg>, evt: Self::Msg) {
        if let TestMsg::Data(value) = evt {
            self.update_state(value);
        }
    }

    fn replay_event(&mut self, _ctx: &Context<Self::Msg>, evt: Self::Msg) {
        if let TestMsg::Data(value) = evt {
            self.update_state(value);
        }
    }

    fn persistence_conf(&self) -> Option<PersistenceConf> {
        let conf = PersistenceConf {
            id: self.id.clone(),
            keyspace: "persist_test".to_string()
        };

        Some(conf)
    }
}

#[test]
#[allow(dead_code)]
fn persist_actor_stop() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new_args(Box::new(PersistActor::new), "0123456789".to_string());
    let actor = system.actor_of(props, "persist").unwrap();

    let (probe, listen) = probe();

    // Send 10 messages with a value of Data(10) each
    // 10 * 10 = 100;
    for _ in 0..10 {
        actor.tell(TestMsg::Data(10), None);
    }

    actor.tell(TestMsg::Probe(TestProbe(probe.clone())), None);
    actor.tell(TestMsg::ReportState, None);

    // We're expecting a value of 100
    p_assert_eq!(listen, 100);

    system.stop(&actor);

    let props = Props::new_args(Box::new(PersistActor::new), "0123456789".to_string());
    let actor = system.actor_of(props, "persist-restart").unwrap();

    actor.tell(TestMsg::Probe(TestProbe(probe)), None);
    actor.tell(TestMsg::ReportState, None);

    // After starting a second actor with the same ID as the first it will replay all events
    // resulting in the expected value of 100
    p_assert_eq!(listen, 100);
}

#[test]
#[allow(dead_code)]
fn persist_actor_failed() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new_args(Box::new(PersistActor::new), "0123456789".to_string());
    let actor = system.actor_of(props, "persist").unwrap();

    let (probe, listen) = probe();

    // Send 10 messages with a value of Data(10) each
    // 10 * 10 = 100;
    for _ in 0..10 {
        actor.tell(TestMsg::Data(10), None);
    }

    actor.tell(TestMsg::Panic, None);

    actor.tell(TestMsg::Probe(TestProbe(probe)), None);
    actor.tell(TestMsg::ReportState, None);

    // After starting a second actor with the same ID as the first it will replay all events
    // resulting in the expected value of 100
    p_assert_eq!(listen, 100);
}