extern crate riker;
use riker::{actors::*, system::SystemCmd};

use std::time::Duration;

#[derive(Default)]
struct Child;

impl Actor for Child {
    type Msg = String;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        println!("child got a message {}", msg);
    }

    fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, _sender: Sender) {
        if let SystemMsg::Command(cmd) = msg {
            match cmd {
                SystemCmd::Stop => ctx.system.stop(ctx.myself()),
                SystemCmd::Restart => {},
            }
        }
    }
}

#[derive(Default)]
struct MyActor {
    child: Option<ActorRef<String>>,
}

#[derive(Debug, Clone)]
enum Command {
    KillChild(String),
    Other(String)
}

// implement the Actor trait
impl Actor for MyActor {
    type Msg = Command;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.child = Some(ctx.actor_of::<Child>("my-child").unwrap());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        match msg {
            Command::KillChild(path) => {
                ctx.myself()
                    .children()
                    .find(|act_ref| act_ref.path().to_string() == path)
                    .map(|act| ctx.stop(act));
            },
            Command::Other(inner_msg) => {
                println!("parent got a message {}", inner_msg);
                self.child.as_ref().unwrap().tell(inner_msg, sender);
            },
        }
    }
}

// start the system and create an actor
fn main() {
    let sys = ActorSystem::new().unwrap();

    println!("Starting actor my-actor");
    let _my_actor = sys.actor_of::<MyActor>("my-actor").unwrap();

    std::thread::sleep(Duration::from_millis(500));
    sys.print_tree();

    println!("Killing actor my-actor");
    let select = sys.select("/user/my-actor").unwrap();
    select.try_tell(Command::KillChild("/user/my-actor/my-child".to_string()), None);
    println!("Actor my-actor should be");
    std::thread::sleep(Duration::from_millis(500));
    sys.print_tree();
}
