extern crate riker;
use riker::actors::*;

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Panic;

#[actor(Panic)]
#[derive(Default)]
struct DumbActor;

impl Actor for DumbActor {
    type Msg = DumbActorMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Panic> for DumbActor {
    type Msg = DumbActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Panic, _sender: Sender) {
        panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
    }
}

// *** Publish test ***
#[actor(SystemEvent)]
#[derive(Default)]
struct SystemActor;

impl Actor for SystemActor {
    type Msg = SystemActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let topic = Topic::from("*");

        println!(
            "{}: pre_start subscribe to topic {:?}",
            ctx.myself.name(),
            topic
        );
        let sub = Box::new(ctx.myself());

        ctx.system.sys_events().tell(
            Subscribe {
                actor: sub,
                topic: "*".into(),
            },
            None,
        );
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }

    fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Sender) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender);
        }
    }
}

impl Receive<SystemEvent> for SystemActor {
    type Msg = SystemActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SystemEvent, _sender: Sender) {
        print!("{}: -> got system msg: {:?} ", ctx.myself.name(), msg);
        match msg {
            SystemEvent::ActorCreated(created) => {
                println!("path: {}", created.actor.path());
            }
            SystemEvent::ActorRestarted(restarted) => {
                println!("path: {}", restarted.actor.path());
            }
            SystemEvent::ActorTerminated(terminated) => {
                println!("path: {}", terminated.actor.path());
            }
        }
    }
}

fn main() {
    let sys = ActorSystem::new().unwrap();

    let _sub = sys.actor_of::<SystemActor>("system-actor").unwrap();

    std::thread::sleep(Duration::from_millis(500));

    println!("Creating dump actor");
    let dumb = sys.actor_of::<DumbActor>("dumb-actor").unwrap();

    // sleep another half seconds to process messages
    std::thread::sleep(Duration::from_millis(500));

    // Force restart of actor
    println!("Send Panic message to dump actor to get restart");
    dumb.tell(Panic, None);
    std::thread::sleep(Duration::from_millis(500));

    println!("Stopping dump actor");
    sys.stop(&dumb);
    std::thread::sleep(Duration::from_millis(500));
    sys.print_tree();
}
