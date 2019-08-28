use async_trait::async_trait;
use futures::executor::block_on;
use riker::actors::*;

#[test]
fn system_create() {
    block_on(async {
        assert!(ActorSystem::new().await.is_ok());
        assert!(ActorSystem::with_name("valid-name").await.is_ok());

        assert!(ActorSystem::with_name("/").await.is_err());
        assert!(ActorSystem::with_name("*").await.is_err());
        assert!(ActorSystem::with_name("/a/b/c").await.is_err());
        assert!(ActorSystem::with_name("@").await.is_err());
        assert!(ActorSystem::with_name("#").await.is_err());
        assert!(ActorSystem::with_name("abc*").await.is_err());
    });
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
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        let props = Props::new_args(ShutdownTest::actor, 1);
        sys.actor_of(props, "test-actor-1").await.unwrap();
        sys.shutdown().await;
    });
}

#[test]
fn system_futures_exec() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        for i in 0..100 {
            let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

            assert_eq!(f.await, format!("some_val_{}", i));
        }
    });
}

#[test]
fn system_futures_panic() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        for _ in 0..100 {
            let _ = sys
                .run(async move {
                    panic!("// TEST PANIC // TEST PANIC // TEST PANIC //");
                })
                .unwrap()
                .await;
        }

        for i in 0..100 {
            let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

            assert_eq!(f.await, format!("some_val_{}", i));
        }
    });
}

#[test]
fn system_load_app_config() {
    block_on(async {
        let sys = ActorSystem::new().await.unwrap();

        assert_eq!(sys.config().get_int("app.some_setting").unwrap() as i64, 1);
    });
}

#[test]
fn system_builder() {
    block_on(async {
        let sys = SystemBuilder::new().name("my-sys").create().await.unwrap();

        sys.shutdown().await;
    });
}
