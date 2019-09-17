use futures::executor::block_on;
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

impl ShutdownTest {
    fn actor(level: u32) -> Self {
        ShutdownTest { level: level }
    }
}

impl Actor for ShutdownTest {
    type Msg = ();

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            let props = Props::new_args(ShutdownTest::actor, self.level + 1);
            ctx.actor_of(props, format!("test-actor-{}", self.level + 1).as_str())
                .unwrap();
        }
    }

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
#[allow(dead_code)]
fn system_when_shutdown() {
    let sys = ActorSystem::new().unwrap();

    let t = sys.when_terminated();

    std::thread::spawn(move || {
        let s = sys.shutdown();
        block_on(s).unwrap();
    });

    block_on(t).unwrap();
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new_args(ShutdownTest::actor, 1);
    let _ = sys.actor_of(props, "test-actor-1").unwrap();

    block_on(sys.shutdown()).unwrap();
}

#[test]
fn system_futures_exec() {
    let sys = ActorSystem::new().unwrap();

    for i in 0..100 {
        let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_futures_panic() {
    let sys = ActorSystem::new().unwrap();

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
    let sys = ActorSystem::new().unwrap();

    assert_eq!(sys.config().get_int("app.some_setting").unwrap() as i64, 1);
}

#[test]
fn system_builder() {
    let sys = SystemBuilder::new().name("my-sys").create().unwrap();

    block_on(sys.shutdown()).unwrap();
}
