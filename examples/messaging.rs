extern crate riker;
use riker::actors::*;

use std::time::Duration;

// Define the messages we'll use
#[derive(Clone, Debug)]
pub struct Add;

#[derive(Clone, Debug)]
pub struct Sub;

#[derive(Clone, Debug)]
pub struct Print;

// Define the Actor and use the 'actor' attribute
// to specify which messages it will receive
#[actor(Add, Sub, Print)]
struct Counter {
    count: u32,
}

impl ActorFactoryArgs<u32> for Counter {
    fn create_args(count: u32) -> Self {
        Self { count }
    }
}

impl Actor for Counter {
    // we used the #[actor] attribute so CounterMsg is the Msg type
    type Msg = CounterMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Add> for Counter {
    type Msg = CounterMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Add, _sender: Sender) {
        self.count += 1;
    }
}

impl Receive<Sub> for Counter {
    type Msg = CounterMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Sub, _sender: Sender) {
        self.count -= 1;
    }
}

impl Receive<Print> for Counter {
    type Msg = CounterMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: Print, _sender: Sender) {
        println!("Total counter value: {}", self.count);
    }
}

#[tokio::main]
async fn main() {
    let backend = tokio::runtime::Handle::current().into();
    let sys = ActorSystem::new(backend).unwrap();

    let actor = sys.actor_of_args::<Counter, _>("counter", 0).unwrap();
    actor.tell(Add, None);
    actor.tell(Add, None);
    actor.tell(Sub, None);
    actor.tell(Print, None);
    for line in sys.print_tree() {
        println!("{}", line);
    }
    // force main to wait before exiting program
    tokio::time::sleep(Duration::from_millis(500)).await;
}
