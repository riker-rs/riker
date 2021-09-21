use futures::executor::block_on;
use riker::actors::*;

#[test]
fn system_create() {
    assert!(ActorSystem::new(ThreadPoolConfig::new(1, 0)).is_ok());
    assert!(ActorSystem::with_name("valid-name", ThreadPoolConfig::new(1, 0)).is_ok());

    assert!(ActorSystem::with_name("/", ThreadPoolConfig::new(1, 0)).is_err());
    assert!(ActorSystem::with_name("*", ThreadPoolConfig::new(1, 0)).is_err());
    assert!(ActorSystem::with_name("/a/b/c", ThreadPoolConfig::new(1, 0)).is_err());
    assert!(ActorSystem::with_name("@", ThreadPoolConfig::new(1, 0)).is_err());
    assert!(ActorSystem::with_name("#", ThreadPoolConfig::new(1, 0)).is_err());
    assert!(ActorSystem::with_name("abc*", ThreadPoolConfig::new(1, 0)).is_err());
}

struct ShutdownTest {
    level: u32,
}

impl ActorFactoryArgs<u32> for ShutdownTest {
    fn create_args(level: u32) -> Self {
        ShutdownTest { level }
    }
}

impl Actor for ShutdownTest {
    type Msg = ();

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if self.level < 10 {
            ctx.actor_of_args::<ShutdownTest, _>(
                format!("test-actor-{}", self.level + 1).as_str(),
                self.level + 1,
            )
            .unwrap();
        }
    }

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

#[test]
#[allow(dead_code)]
fn system_shutdown() {
    let sys = ActorSystem::new(ThreadPoolConfig::new(1, 0)).unwrap();

    let _ = sys
        .actor_of_args::<ShutdownTest, _>("test-actor-1", 1)
        .unwrap();

    block_on(sys.shutdown()).unwrap();
}

#[test]
fn system_futures_exec() {
    let sys = ActorSystem::new(ThreadPoolConfig::new(1, 0)).unwrap();

    for i in 0..100 {
        let f = sys.run(async move { format!("some_val_{}", i) }).unwrap();

        assert_eq!(block_on(f), format!("some_val_{}", i));
    }
}

#[test]
fn system_futures_panic() {
    let sys = ActorSystem::new(ThreadPoolConfig::new(1, 0)).unwrap();

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
fn system_builder() {
    let sys = SystemBuilder::new().create(ThreadPoolConfig::new(1, 0)).unwrap();
    block_on(sys.shutdown()).unwrap();

    let sys = SystemBuilder::new().name("my-sys").create(ThreadPoolConfig::new(1, 0)).unwrap();
    block_on(sys.shutdown()).unwrap();
}
