extern crate riker;
use riker::actors::*;

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Panic;

#[derive(Default)]
struct DumbActor;

impl Actor for DumbActor {
    type Msg = ();

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[actor(Panic)]
#[derive(Default)]
struct PanicActor;

impl Actor for PanicActor {
    type Msg = PanicActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.actor_of::<DumbActor>("child_a").unwrap();

        ctx.actor_of::<DumbActor>("child_b").unwrap();

        ctx.actor_of::<DumbActor>("child_c").unwrap();

        ctx.actor_of::<DumbActor>("child_d").unwrap();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Panic> for PanicActor {
    type Msg = PanicActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

// Test Restart Strategy
#[actor(Panic)]
#[derive(Default)]
struct RestartSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl Actor for RestartSup {
    type Msg = RestartSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.actor_to_fail = ctx.actor_of::<PanicActor>("actor-to-fail").ok();
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender)
    }
}

impl Receive<Panic> for RestartSup {
    type Msg = RestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None);
    }
}

fn main() {
    let sys = ActorSystem::new().unwrap();

    let sup = sys.actor_of::<RestartSup>("supervisor").unwrap();
    // println!("Child not added yet");
    // sys.print_tree();

    println!("Before panic we see supervisor and actor that will panic!");
    std::thread::sleep(Duration::from_millis(500));
    sys.print_tree();

    sup.tell(Panic, None);
    std::thread::sleep(Duration::from_millis(500));
    println!("We should see panic printed, but we still alive and panic actor gone!");
    sys.print_tree();
}
