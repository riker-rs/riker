#![feature(
        async_await,
        await_macro,
        futures_api,
        arbitrary_self_types
)]

use riker::actors::*;
use futures::executor::block_on;

#[test]
fn system_create() {
    assert!(ActorSystem::new().is_ok());
    assert!(ActorSystem::with_name("valid-name").is_ok());

    assert!(ActorSystem::with_name("/").is_err());
    assert!(ActorSystem::with_name("*").is_err());
    assert!(ActorSystem::with_name("/a/b/c").is_err());
    assert!(ActorSystem::with_name("@").is_err());
    assert!(ActorSystem::with_name("#").is_err());
    assert!(ActorSystem::with_name("abc*").is_err());
}

struct ShutdownTest {
    level: u32,
}

impl ShutdownTest {
    fn actor(level: u32) -> Self {
        ShutdownTest {
            level: level
        }
    }
}

impl Actor for ShutdownTest {
    type Msg = ();
    type Evt = ();

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            let props = Props::new_args(Box::new(ShutdownTest::actor), self.level + 1);
            ctx.actor_of(props,
                        format!("test-actor-{}", self.level + 1)
                        .as_str()
            ).unwrap();
        }
    }

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new_args(Box::new(ShutdownTest::actor), 1);
    let _ = sys.actor_of(props, "test-actor-1").unwrap();

    block_on(sys.shutdown()).unwrap();
}

// #[test]
// fn system_execute_futures() {
//     let model: DefaultModel<TestMsg> = DefaultModel::new();
//     let system = ActorSystem::new(&model).unwrap();

//     for i in 0..100 {
//         let handle = system.execute(async move {
//             format!("some_val_{}", i)
//         });
        
//         assert_eq!(block_on(handle), format!("some_val_{}", i));
//     }
// }

// #[test]
// fn system_panic_futures() {
//     let model: DefaultModel<TestMsg> = DefaultModel::new();
//     let system = ActorSystem::new(&model).unwrap();

//     for _ in 0..100 {
//         let _ = system.execute(async move {
//             panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
//         });
//     }

//     for i in 0..100 {
//         let handle = system.execute(async move {
//             format!("some_val_{}", i)
//         });
        
//         assert_eq!(block_on(handle), format!("some_val_{}", i));
//     }
// }

#[test]
fn system_load_app_config() {
    let sys = ActorSystem::new().unwrap();
    
    assert_eq!(sys.config()
                    .get_int("app.some_setting")
                    .unwrap() as i64, 1);
}
