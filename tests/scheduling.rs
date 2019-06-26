#[macro_use]
extern crate riker_testkit;

use riker::actors::*;

use riker_testkit::probe::{Probe, ProbeReceive};
use riker_testkit::probe::channel::{probe, ChannelProbe};

use chrono::{Utc, Duration as CDuration};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[derive(Clone, Debug)]
pub struct SomeMessage;

#[actor(TestProbe, SomeMessage)]
struct ScheduleOnce {
    probe: Option<TestProbe>,
}

impl ScheduleOnce {
    fn new() -> Self {
        ScheduleOnce {
            probe: None
        }
    }
}

impl Actor for ScheduleOnce {
    type Msg = ScheduleOnceMsg;
    type Evt = ();

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    fn receive(&mut self,
                ctx: &Context<ScheduleOnceMsg>,
                msg: TestProbe,
                sender: Sender) {

        self.probe = Some(msg);
        // reschedule an Empty to be sent to myself()
        ctx.schedule_once(Duration::from_millis(200),
                            ctx.myself(),
                            None,
                            SomeMessage);
    }
}

impl Receive<SomeMessage> for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    fn receive(&mut self,
                ctx: &Context<ScheduleOnceMsg>,
                msg: SomeMessage,
                sender: Sender) {

        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn schedule_once() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new(Box::new(ScheduleOnce::new));
    let actor = sys.actor_of(props, "schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe
    sys.schedule_once(Duration::from_millis(200),
                        actor,
                        None,
                        TestProbe(probe));
    p_assert_eq!(listen, ());
}

#[test]
fn schedule_at_time() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new(Box::new(ScheduleOnce::new));
    let actor = sys.actor_of(props, "schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe at a specific time
    let schedule_at = Utc::now() + CDuration::milliseconds(200);
    sys.schedule_at_time(schedule_at,
                            actor,
                            None,
                            TestProbe(probe));
    p_assert_eq!(listen, ());
}

// *** Schedule repeat test ***
#[actor(TestProbe, SomeMessage)]
struct ScheduleRepeat {
    probe: Option<TestProbe>,
    counter: u32,
    schedule_id: Option<Uuid>,
}

impl ScheduleRepeat {
    fn new() -> Self {
        ScheduleRepeat {
            probe: None,
            counter: 0,
            schedule_id: None
        }
    }
}

impl Actor for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;
    type Evt = ();
    
    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {

        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    fn receive(&mut self,
                ctx: &Context<Self::Msg>,
                msg: TestProbe,
                sender: Sender) {

        self.probe = Some(msg);
        // schedule Message to be repeatedly sent to myself
        // and store the job id to cancel it later
        let id = ctx.schedule(Duration::from_millis(200),
                                Duration::from_millis(200),
                                    ctx.myself(),
                                    None,
                                    SomeMessage);
        self.schedule_id = Some(id);
    }
}

impl Receive<SomeMessage> for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    fn receive(&mut self,
                ctx: &Context<Self::Msg>,
                msg: SomeMessage,
                sender: Sender) {

        if self.counter == 5 {
            ctx.cancel_schedule(self.schedule_id.unwrap());
            self.probe.as_ref().unwrap().0.event(());
        } else {
            self.counter += 1;
        }
    }
}

#[test]
fn schedule_repeat() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new(Box::new(ScheduleRepeat::new));
    let actor = sys.actor_of(props, "schedule-repeat").unwrap();

    let (probe, listen) = probe();

    actor.tell(TestProbe(probe), None);
    
    p_assert_eq!(listen, ());
}
