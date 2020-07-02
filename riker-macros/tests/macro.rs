use riker::actors::*;

#[actor(String, u32)]
#[derive(Clone, Default)]
struct NewActor;

impl Actor for NewActor {
    type Msg = NewActorMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        ctx.stop(&ctx.myself);
    }
}

impl Receive<u32> for NewActor {
    type Msg = NewActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: u32, _sender: Option<BasicActorRef>) {
        println!("u32");
    }
}

impl Receive<String> for NewActor {
    type Msg = NewActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: String, _sender: Option<BasicActorRef>) {
        println!("String");
    }
}

#[test]
fn run_derived_actor() {
    let sys = ActorSystem::new().unwrap();

    let act = sys.actor_of::<NewActor>("act").unwrap();

    let msg = NewActorMsg::U32(1);
    act.tell(msg, None);

    // wait until all direct children of the user root are terminated
    while sys.user_root().has_children() {
        // in order to lower cpu usage, sleep here
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[actor(String)]
#[derive(Clone, Default)]
struct GenericActor<A: Send + 'static, B>
    where B: Send + 'static,
{
    a: A,
    b: B,
}

impl<A: Send + 'static, B: Send + 'static> Actor for GenericActor<A, B> {
    type Msg = GenericActorMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        ctx.stop(&ctx.myself);
    }
}

impl<A: Send + 'static, B: Send + 'static> Receive<String> for GenericActor<A, B> {
    type Msg = GenericActorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: String, _sender: Option<BasicActorRef>) {
        println!("String");
    }
}

#[test]
fn run_derived_generic_actor() {
    let sys = ActorSystem::new().unwrap();

    let act = sys.actor_of::<GenericActor<(), ()>>("act").unwrap();

    let msg = GenericActorMsg::String("test".to_string());
    act.tell(msg, None);

    // wait until all direct children of the user root are terminated
    while sys.user_root().has_children() {
        // in order to lower cpu usage, sleep here
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}