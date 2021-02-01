use riker::actors::*;
use riker::Message;

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
where
    B: Send + 'static,
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

#[derive(Clone, Debug)]
pub struct GenericMessage<T> {
    inner: T,
}

#[actor(GenericMessage<String>)]
#[derive(Clone, Default)]
struct GenericMsgActor;

impl Actor for GenericMsgActor {
    type Msg = GenericMsgActorMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        ctx.stop(&ctx.myself);
    }
}

impl Receive<GenericMessage<String>> for GenericMsgActor {
    type Msg = GenericMsgActorMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        msg: GenericMessage<String>,
        _sender: Option<BasicActorRef>,
    ) {
        println!("{}", msg.inner);
    }
}

#[test]
fn run_generic_message_actor() {
    let sys = ActorSystem::new().unwrap();

    let act = sys.actor_of::<GenericMsgActor>("act").unwrap();

    let msg = GenericMessage {
        inner: "test".to_string(),
    };
    act.tell(msg, None);

    // wait until all direct children of the user root are terminated
    while sys.user_root().has_children() {
        // in order to lower cpu usage, sleep here
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

mod test_mod {
    #[derive(Clone, Debug)]
    pub struct GenericMessage<T> {
        pub inner: T,
    }

    #[derive(Clone, Debug)]
    pub struct Message;
}

#[actor(test_mod::GenericMessage<String>, test_mod::Message)]
#[derive(Clone, Default)]
struct PathMsgActor;

impl Actor for PathMsgActor {
    type Msg = PathMsgActorMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        ctx.stop(&ctx.myself);
    }
}

impl Receive<test_mod::GenericMessage<String>> for PathMsgActor {
    type Msg = PathMsgActorMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        msg: test_mod::GenericMessage<String>,
        _sender: Option<BasicActorRef>,
    ) {
        println!("{}", msg.inner);
    }
}

impl Receive<test_mod::Message> for PathMsgActor {
    type Msg = PathMsgActorMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        _msg: test_mod::Message,
        _sender: Option<BasicActorRef>,
    ) {
        println!("message");
    }
}

#[test]
fn run_path_message_actor() {
    let sys = ActorSystem::new().unwrap();

    let act = sys.actor_of::<PathMsgActor>("act").unwrap();

    let msg = PathMsgActorMsg::TestModMessage(test_mod::Message);
    act.tell(msg, None);

    let generic_msg = PathMsgActorMsg::TestModGenericMessage(test_mod::GenericMessage {
        inner: "test".to_string(),
    });
    act.tell(generic_msg, None);

    // wait until all direct children of the user root are terminated
    while sys.user_root().has_children() {
        // in order to lower cpu usage, sleep here
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[derive(Clone, Debug)]
pub struct KnownMessageType;

#[derive(Clone, Debug)]
struct GenericStruct<T>
where
    T: Message + From<KnownMessageType>,
{
    actor: ActorRef<T>,
}

impl<T> GenericStruct<T>
where
    T: Message + From<KnownMessageType>,
{
    fn msg_actor(&self, msg: KnownMessageType) {
        self.actor.tell(msg, None)
    }
}

#[derive(Clone, Debug)]
pub struct OtherMessageType;

#[actor(KnownMessageType, OtherMessageType)]
#[derive(Clone, Debug, Default)]
struct AnyActor;

impl Actor for AnyActor {
    type Msg = AnyActorMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
        ctx.stop(&ctx.myself);
    }
}

impl Receive<KnownMessageType> for AnyActor {
    type Msg = AnyActorMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        msg: KnownMessageType,
        _sender: Option<BasicActorRef>,
    ) {
        println!("Received {:?}", msg);
    }
}

impl Receive<OtherMessageType> for AnyActor {
    type Msg = AnyActorMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        _msg: OtherMessageType,
        _sender: Option<BasicActorRef>,
    ) {
    }
}

#[test]
fn tell_generic_actor_ref() {
    let sys = ActorSystem::new().unwrap();
    let actor = sys.actor_of::<AnyActor>("any").unwrap();
    let some_struct = GenericStruct { actor };
    some_struct.msg_actor(KnownMessageType);
    // wait until all direct children of the user root are terminated
    while sys.user_root().has_children() {
        // in order to lower cpu usage, sleep here
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
