#![feature(
        async_await,
        await_macro,
        futures_api,
        arbitrary_self_types
)]

use riker::actors::*;
use riker_default::DefaultModel;

use futures::executor::block_on;

struct ShutdownTestActor {
    level: u32,
}

impl ShutdownTestActor {
    fn new(level: u32) -> BoxActor<TestMsg> {
        let actor = ShutdownTestActor {
            level: level
        };

        Box::new(actor)
    }
}

impl Actor for ShutdownTestActor {
    type Msg = TestMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            let props = Props::new_args(Box::new(ShutdownTestActor::new), self.level + 1);
            ctx.actor_of(props, format!("test-actor-{}", self.level + 1).as_str()).unwrap();
        }
    }

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
}

#[derive(Clone, Debug)]
struct TestMsg(());

impl Into<ActorMsg<TestMsg>> for TestMsg {
    fn into(self) -> ActorMsg<TestMsg> {
        ActorMsg::User(self)
    }
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    let props = Props::new_args(Box::new(ShutdownTestActor::new), 1);
    let _ = system.actor_of(props, "test-actor-1").unwrap();

    block_on(system.shutdown());
}


#[test]
fn system_guardian_mailboxes() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();
    
    let user_root = system.user_root();
    user_root.tell(TestMsg(()), None);
    user_root.tell(TestMsg(()), None);

    // We're testing a system actor so wait for 1s after
    // sending the system message to give time to panic if it fails
    std::thread::sleep(std::time::Duration::from_millis(1000));
}

#[test]
fn system_execute_futures() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    for i in 0..100 {
        let handle = system.execute(async move {
            format!("some_val_{}", i)
        });
        
        assert_eq!(block_on(handle), format!("some_val_{}", i));
    }
}

#[test]
fn system_panic_futures() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();

    for _ in 0..100 {
        system.execute(async move {
            panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
        });
    }

    for i in 0..100 {
        let handle = system.execute(async move {
            format!("some_val_{}", i)
        });
        
        assert_eq!(block_on(handle), format!("some_val_{}", i));
    }
}

#[test]
fn system_load_app_config() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();
    
    assert_eq!(system.config().get_int("app.some_setting").unwrap() as i64, 1);
}
