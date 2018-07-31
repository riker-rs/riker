extern crate riker;
extern crate riker_default;
#[macro_use]
extern crate riker_testkit;
extern crate uuid;

use std::time::{Duration, SystemTime};
use uuid::Uuid;

use riker::actors::*;
use riker_default::DefaultModel;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

#[derive(Clone, Debug)]
enum TestMsg {
    Probe(TestProbe),
    Empty,
}
type TestProbe = ChannelProbe<(), ()>;

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

struct ScheduleOnceActor {
    probe: Option<TestProbe>,
}

impl ScheduleOnceActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = ScheduleOnceActor {
            probe: None
        };

        Box::new(actor)
    }
}

impl Actor for ScheduleOnceActor {
    type Msg = TestMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Probe(probe) => {
                self.probe = Some(probe);
                // reschedule an Empty to be sent to myself
                // to test schedule works from Context
                ctx.schedule_once(Duration::from_millis(200),
                                    ctx.myself(),
                                    None,
                                    TestMsg::Empty);
            }
            TestMsg::Empty => {
                self.probe.event(());
            }
        }
    }
}

#[test]
fn schedule_once() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(ScheduleOnceActor::new));
    let actor = system.actor_of(props, "schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe
    system.schedule_once(Duration::from_millis(200),
                        actor,
                        None,
                        TestMsg::Probe(probe));
    p_assert_eq!(listen, ());
}

#[test]
fn schedule_at_time() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(ScheduleOnceActor::new));
    let actor = system.actor_of(props, "schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe at a specific time
    let schedule_at = SystemTime::now() + Duration::from_millis(200);
    system.schedule_at_time(schedule_at,
                            actor,
                            None,
                            TestMsg::Probe(probe));
    p_assert_eq!(listen, ());
}

// *** Schedule repeat test ***
struct ScheduleRepeatActor {
    probe: Option<TestProbe>,
    counter: u32,
    schedule_id: Option<Uuid>,
}

impl ScheduleRepeatActor {
    fn new() -> BoxActor<TestMsg> {
        let actor = ScheduleRepeatActor {
            probe: None,
            counter: 0,
            schedule_id: None
        };

        Box::new(actor)
    }
}

impl Actor for ScheduleRepeatActor {
    type Msg = TestMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
        match msg {
            TestMsg::Probe(probe) => {
                self.probe = Some(probe);
                // schedule Message to be repeatedly sent to myself
                // and store the job id to cancel it later
                let id = ctx.schedule(Duration::from_millis(200),
                                Duration::from_millis(100),                                
                                ctx.myself(),
                                None,
                                TestMsg::Empty);

                self.schedule_id = Some(id);
            }
            TestMsg::Empty => {
                if self.counter == 5 {
                    ctx.cancel_schedule(self.schedule_id.unwrap());
                    self.probe.event(());
                } else {
                    self.counter += 1;
                }
            }
        }
    }
}

#[test]
fn schedule_repeat() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new(Box::new(ScheduleRepeatActor::new));
    let actor = system.actor_of(props, "schedule-repeat").unwrap();

    let (probe, listen) = probe();

    actor.tell(TestMsg::Probe(probe), None);
    
    p_assert_eq!(listen, ());
}
