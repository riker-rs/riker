#![feature(
async_await,
await_macro,
arbitrary_self_types
)]

use futures::{
    executor::block_on
};

use riker::actors::*;

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

impl ActorFactoryArgs<u32> for ShutdownTest {
    fn create_args(level: u32) -> BoxActorProd<Self> {
        Props::new_args(|level| ShutdownTest {
            level
        }, level)
    }
}

impl Actor for ShutdownTest {
    type Msg = ();

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            ctx.actor_of_args::<ShutdownTest, _>(
                format!("test-actor-{}", self.level + 1)
                    .as_str(),
                self.level + 1,
            ).unwrap();
        }
    }

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let sys = ActorSystem::new().unwrap();

    let _ = sys.actor_of_args::<ShutdownTest, _>("test-actor-1", 1).unwrap();

    block_on(sys.shutdown()).unwrap();
}

#[test]
fn system_futures_exec() {
    let sys = ActorSystem::new().unwrap();

    for i in 0..100 {
        let f = sys.run(async move {
            format!("some_val_{}", i)
        }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_futures_panic() {
    let sys = ActorSystem::new().unwrap();

    for _ in 0..100 {
        let _ = sys.run(async move {
            panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
        }).unwrap();
    }

    for i in 0..100 {
        let f = sys.run(async move {
            format!("some_val_{}", i)
        }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_load_app_config() {
    let sys = ActorSystem::new().unwrap();

    assert_eq!(sys.config()
                   .get_int("app.some_setting")
                   .unwrap() as i64, 1);
}

#[test]
fn system_builder() {
    let sys = SystemBuilder::new()
        .name("my-sys")
        .create()
        .unwrap();

    block_on(sys.shutdown()).unwrap();
}
