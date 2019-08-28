use async_trait::async_trait;
use futures::executor::block_on;
use riker::actors::*;

#[test]
fn system_create() {
    assert!(block_on(ActorSystem::new()).is_ok());
    assert!(block_on(ActorSystem::with_name("valid-name")).is_ok());

    assert!(block_on(ActorSystem::with_name("/")).is_err());
    assert!(block_on(ActorSystem::with_name("*")).is_err());
    assert!(block_on(ActorSystem::with_name("/a/b/c")).is_err());
    assert!(block_on(ActorSystem::with_name("@")).is_err());
    assert!(block_on(ActorSystem::with_name("#")).is_err());
    assert!(block_on(ActorSystem::with_name("abc*")).is_err());
}

struct ShutdownTest {
    level: u32,
}

impl ShutdownTest {
    fn actor(level: u32) -> Self {
        ShutdownTest { level: level }
    }
}

#[async_trait]
impl Actor for ShutdownTest {
    type Msg = ();

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            let props = Props::new_args(ShutdownTest::actor, self.level + 1);
            let name = format!("test-actor-{}", self.level + 1);
            ctx.actor_of(props, &name).await.unwrap();
        }
    }

    async fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) { /* empty */ }
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let sys = block_on(ActorSystem::new()).unwrap();

    let props = Props::new_args(ShutdownTest::actor, 1);
    let _ = block_on(sys.actor_of(props, "test-actor-1")).unwrap();

    let _ = block_on(sys.shutdown());
}

#[test]
fn system_futures_exec() {
    let sys = block_on(ActorSystem::new()).unwrap();

    for i in 0..100 {
        let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_futures_panic() {
    let sys = block_on(ActorSystem::new()).unwrap();

    for _ in 0..100 {
        let _ = sys
            .run(async move {
                panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
            })
            .unwrap();
    }

    for i in 0..100 {
        let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_load_app_config() {
    let sys = block_on(ActorSystem::new()).unwrap();

    assert_eq!(sys.config().get_int("app.some_setting").unwrap() as i64, 1);
}

#[test]
fn system_builder() {
    let sys = block_on(SystemBuilder::new().name("my-sys").create()).unwrap();

    let _ = block_on(sys.shutdown());
}
