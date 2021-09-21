extern crate riker;
use riker::actors::*;
use riker::system::ActorSystem;

use std::time::Duration;

// a simple minimal actor for use in tests
// #[actor(TestProbe)]
#[derive(Default, Debug)]
struct Child;

impl Actor for Child {
    type Msg = String;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        println!("{}: {:?} -> got msg: {}", ctx.myself.name(), self, msg);
    }
}

#[derive(Clone, Default, Debug)]
struct SelectTest;

impl Actor for SelectTest {
    type Msg = String;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // create first child actor
        let _ = ctx.actor_of::<Child>("child_a").unwrap();

        // create second child actor
        let _ = ctx.actor_of::<Child>("child_b").unwrap();
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        println!("{}: {:?} -> got msg: {}", ctx.myself.name(), self, msg);
        // up and down: ../select-actor/child_a
        let path = "../select-actor/child_a";
        println!("{}: {:?} -> path: {}", ctx.myself.name(), self, path);
        let sel = ctx.select(path).unwrap();
        sel.try_tell(path.to_string(), None);

        // child: child_a
        let path = "child_a";
        println!("{}: {:?} -> path: {}", ctx.myself.name(), self, path);
        let sel = ctx.select(path).unwrap();
        sel.try_tell(path.to_string(), None);

        // absolute: /user/select-actor/child_a
        let path = "/user/select-actor/child_a";
        println!("{}: {:?} -> path: {}", ctx.myself.name(), self, path);
        let sel = ctx.select(path).unwrap();
        sel.try_tell(path.to_string(), None);

        // absolute all: /user/select-actor/*
        let path = "/user/select-actor/*";
        println!("{}: {:?} -> path: {}", ctx.myself.name(), self, path);
        let sel = ctx.select(path).unwrap();
        sel.try_tell(path.to_string(), None);

        // all: *
        let path = "*";
        println!("{}: {:?} -> path: {}", ctx.myself.name(), self, path);
        let sel = ctx.select(path).unwrap();
        sel.try_tell(path.to_string(), None);
    }
}

fn main() {
    let sys = ActorSystem::new(ThreadPoolConfig::new(1, 0)).unwrap();

    let actor = sys.actor_of::<SelectTest>("select-actor").unwrap();

    actor.tell("msg for select-actor", None);

    std::thread::sleep(Duration::from_millis(500));

    sys.print_tree();
}
