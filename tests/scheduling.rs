#[macro_use]
extern crate riker_testkit;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

use chrono::{Duration as CDuration, Utc};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TestProbe(ChannelProbe<(), ()>);

#[derive(Clone, Debug)]
pub struct SomeMessage;

#[actor(TestProbe, SomeMessage)]
#[derive(Default)]
struct ScheduleOnce {
    probe: Option<TestProbe>,
}

impl Actor for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for ScheduleOnce {
    fn receive(&mut self, ctx: &Context<ScheduleOnceMsg>, msg: TestProbe, _sender: Sender) {
        self.probe = Some(msg);
        // reschedule an Empty to be sent to myself()
        ctx.schedule_once(Duration::from_millis(200), ctx.myself(), None, SomeMessage);
    }
}

impl Receive<SomeMessage> for ScheduleOnce {
    fn receive(&mut self, _ctx: &Context<ScheduleOnceMsg>, _msg: SomeMessage, _sender: Sender) {
        self.probe.as_ref().unwrap().0.event(());
    }
}

#[test]
fn schedule_once() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<ScheduleOnce>("schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe
    sys.schedule_once(Duration::from_millis(200), actor, None, TestProbe(probe));
    p_assert_eq!(listen, ());
}

#[test]
fn schedule_at_time() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<ScheduleOnce>("schedule-once").unwrap();

    let (probe, listen) = probe();

    // use scheduler to set up probe at a specific time
    let schedule_at = Utc::now() + CDuration::milliseconds(200);
    sys.schedule_at_time(schedule_at, actor, None, TestProbe(probe));
    p_assert_eq!(listen, ());
}

// *** Schedule repeat test ***
#[actor(TestProbe, SomeMessage)]
#[derive(Default)]
struct ScheduleRepeat {
    probe: Option<TestProbe>,
    counter: u32,
    schedule_id: Option<Uuid>,
}

impl Actor for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<TestProbe> for ScheduleRepeat {
    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
        self.probe = Some(msg);
        // schedule Message to be repeatedly sent to myself
        // and store the job id to cancel it later
        let id = ctx.schedule(
            Duration::from_millis(200),
            Duration::from_millis(200),
            ctx.myself(),
            None,
            SomeMessage,
        );
        self.schedule_id = Some(id);
    }
}

impl Receive<SomeMessage> for ScheduleRepeat {
    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: SomeMessage, _sender: Sender) {
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

    let actor = sys.actor_of::<ScheduleRepeat>("schedule-repeat").unwrap();

    let (probe, listen) = probe();

    actor.tell(TestProbe(probe), None);

    p_assert_eq!(listen, ());
}
