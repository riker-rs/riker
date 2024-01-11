use futures::executor::block_on;
use futures::future::RemoteHandle;
use riker::actors::*;
use riker_patterns::ask::ask;

#[derive(Default)]
struct EchoActor;

impl Actor for EchoActor {
    type Msg = String;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        sender
            .unwrap()
            .try_tell(msg, Some(ctx.myself().into()))
            .unwrap();
    }
}

#[test]
fn ask_actor() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys.actor_of::<EchoActor>("me").unwrap();

    let msg = "hello".to_string();

    let res: RemoteHandle<String> = ask(&sys, &actor, msg.clone());
    let res = block_on(res);

    assert_eq!(res, msg);
}

#[test]
fn stress_test() {
    #[derive(Debug, Clone)]
    enum Protocol {
        Foo,
        FooResult,
    }

    let system: ActorSystem = ActorSystem::new().unwrap();

    #[derive(Default)]
    struct FooActor;

    impl Actor for FooActor {
        type Msg = Protocol;

        fn recv(&mut self, context: &Context<Self::Msg>, _: Self::Msg, sender: Sender) {
            sender
                .unwrap()
                .try_tell(Protocol::FooResult, Some(context.myself().into()))
                .unwrap();
        }
    }

    let actor = system.actor_of::<FooActor>("foo").unwrap();

    for _i in 1..10_000 {
        let a: RemoteHandle<Protocol> = ask(&system, &actor, Protocol::Foo);
        block_on(a);
    }
}
