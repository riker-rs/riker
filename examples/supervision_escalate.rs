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

#[actor(Panic)]
#[derive(Default)]
struct EscalateSup {
    actor_to_fail: Option<ActorRef<PanicActorMsg>>,
}

impl Actor for EscalateSup {
    type Msg = EscalateSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.actor_to_fail = ctx.actor_of::<PanicActor>("actor-to-fail").ok();
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Escalate
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        // match msg {
        //     // We just resend the messages to the actor that we're concerned about testing
        //     TestMsg::Panic => self.actor_to_fail.try_tell(msg, None).unwrap(),
        //     TestMsg::Probe(_) => self.actor_to_fail.try_tell(msg, None).unwrap(),
        // };
    }
}

impl Receive<Panic> for EscalateSup {
    type Msg = EscalateSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.actor_to_fail.as_ref().unwrap().tell(Panic, None);
    }
}

#[actor(Panic)]
#[derive(Default)]
struct EscRestartSup {
    escalator: Option<ActorRef<EscalateSupMsg>>,
}

impl Actor for EscRestartSup {
    type Msg = EscRestartSupMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.escalator = ctx.actor_of::<EscalateSup>("escalate-supervisor").ok();
    }

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        // match msg {
        //     // We resend the messages to the parent of the actor that is/has panicked
        //     TestMsg::Panic => self.escalator.try_tell(msg, None).unwrap(),
        //     TestMsg::Probe(_) => self.escalator.try_tell(msg, None).unwrap(),
        // };
    }
}

impl Receive<Panic> for EscRestartSup {
    type Msg = EscRestartSupMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        self.escalator.as_ref().unwrap().tell(Panic, None);
    }
}

#[tokio::main]
async fn main() {
    let backend = tokio::runtime::Handle::current().into();
    let sys = ActorSystem::new(backend).unwrap();

    let sup = sys.actor_of::<EscRestartSup>("supervisor").unwrap();

    println!("Before panic we see supervisor and actor that will panic!");
    tokio::time::sleep(Duration::from_millis(500)).await;
    sys.print_tree();

    sup.tell(Panic, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("We should see panic printed, but we still alive and panic actor still here!");
    sys.print_tree();
}
