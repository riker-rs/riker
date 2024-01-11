extern crate riker;
use riker::actors::*;

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SomeMessage;

#[actor(SomeMessage)]
#[derive(Default)]
struct DumbActor;

impl Actor for DumbActor {
    type Msg = DumbActorMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<SomeMessage> for DumbActor {
    type Msg = DumbActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SomeMessage, _sender: Sender) {
        println!("{}: -> got msg: {:?} ", ctx.myself.name(), msg);
    }
}

// *** Publish test ***
#[actor(DeadLetter)]
#[derive(Default)]
struct DeadLetterActor;

impl Actor for DeadLetterActor {
    type Msg = DeadLetterActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let topic = Topic::from("*");

        println!(
            "{}: pre_start subscribe to topic {:?}",
            ctx.myself.name(),
            topic
        );
        let sub = Box::new(ctx.myself());

        ctx.system
            .dead_letters()
            .tell(Subscribe { actor: sub, topic }, None);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<DeadLetter> for DeadLetterActor {
    type Msg = DeadLetterActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: DeadLetter, _sender: Sender) {
        println!("{}: -> got msg: {:?} ", ctx.myself.name(), msg);
    }
}

fn main() {
    let sys = ActorSystem::new().unwrap();

    let _sub = sys.actor_of::<DeadLetterActor>("system-actor").unwrap();

    std::thread::sleep(Duration::from_millis(500));

    println!("Creating dump actor");
    let dumb = sys.actor_of::<DumbActor>("dumb-actor").unwrap();

    println!("Stopping dump actor");
    sys.stop(&dumb);
    std::thread::sleep(Duration::from_millis(500));

    println!("Sending SomeMessage to stopped actor");
    dumb.tell(SomeMessage, None);
    std::thread::sleep(Duration::from_millis(500));
    sys.print_tree();
}
