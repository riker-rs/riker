#[macro_use]
extern crate riker_testkit;

use async_trait::async_trait;

use riker::actors::*;

use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

use chrono::{Duration as CDuration, Utc};
use std::time::Duration;
use uuid::Uuid;
use futures::executor::block_on;

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
        ScheduleOnce { probe: None }
    }
}

#[async_trait]
impl Actor for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

#[async_trait]
impl Receive<TestProbe> for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    async fn receive(&mut self, ctx: &Context<ScheduleOnceMsg>, msg: TestProbe, _sender: Sender) {
        self.probe = Some(msg);
        // reschedule an Empty to be sent to myself()
        ctx.schedule_once(Duration::from_millis(200), ctx.myself(), None, SomeMessage);
    }
}

#[async_trait]
impl Receive<SomeMessage> for ScheduleOnce {
    type Msg = ScheduleOnceMsg;

    async fn receive(&mut self, _ctx: &Context<ScheduleOnceMsg>, _msg: SomeMessage, _sender: Sender) {
        self.probe.as_mut().unwrap().0.event(()).await;
    }
}

#[test]
fn schedule_once() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(ScheduleOnce::new);
        let actor = sys.actor_of(props, "schedule-once").await.unwrap();

        let (probe, mut listen) = probe();

        // use scheduler to set up probe
        sys.schedule_once(Duration::from_millis(200), actor, None, TestProbe(probe));
        p_assert_eq!(listen, ());
    });
}

#[test]
fn schedule_at_time() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(ScheduleOnce::new);
        let actor = sys.actor_of(props, "schedule-once").await.unwrap();

        let (probe, mut listen) = probe();

        // use scheduler to set up probe at a specific time
        let schedule_at = Utc::now() + CDuration::milliseconds(200);
        sys.schedule_at_time(schedule_at, actor, None, TestProbe(probe));
        p_assert_eq!(listen, ());
    });
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
            schedule_id: None,
        }
    }
}

#[async_trait]
impl Actor for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<TestProbe> for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    async fn receive(&mut self, ctx: &Context<Self::Msg>, msg: TestProbe, _sender: Sender) {
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

#[async_trait]
impl Receive<SomeMessage> for ScheduleRepeat {
    type Msg = ScheduleRepeatMsg;

    async fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: SomeMessage, _sender: Sender) {
        if self.counter == 5 {
            ctx.cancel_schedule(self.schedule_id.unwrap());
            self.probe.as_mut().unwrap().0.event(()).await;
        } else {
            self.counter += 1;
        }
    }
}

#[test]
fn schedule_repeat() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new(ScheduleRepeat::new);
        let actor = sys.actor_of(props, "schedule-repeat").await.unwrap();

        let (probe, mut listen) = probe();

        actor.tell(TestProbe(probe), None).await;

        p_assert_eq!(listen, ());
    });
}
