extern crate riker;
extern crate riker_default;

extern crate futures;

use riker::actors::*;
use riker_default::DefaultModel;

use futures::future::*;
use futures::executor::block_on;

// todo add test documentation
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

    // std::thread::sleep(std::time::Duration::from_millis(2000));
    block_on(system.shutdown()).unwrap();
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
    
    let fa = lazy(|_| {
        ok::<String, ()>("some_val".to_string())
    });

    let fb = lazy(|_| {
        ok::<String, ()>("some_val".to_string())
    });

    let fc = lazy(|_| {
        ok::<String, ()>("some_val".to_string())
    });

    let resa = block_on(system.execute(fb)).unwrap();
    let resb = block_on(system.execute(fc)).unwrap();
    let resc = block_on(system.execute(fa)).unwrap();

    assert_eq!(resa, "some_val".to_string());
    assert_eq!(resb, "some_val".to_string());
    assert_eq!(resc, "some_val".to_string());
}

#[test]
fn system_load_app_config() {
    let model: DefaultModel<TestMsg> = DefaultModel::new();
    let system = ActorSystem::new(&model).unwrap();
    
    assert_eq!(system.config().get_int("app.some_setting").unwrap() as i64, 1);
}
