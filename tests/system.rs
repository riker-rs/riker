use tezedge_actor_system::actors::*;
use tokio::runtime::Handle;

#[tokio::test]
async fn system_create() {
    let backend = || Handle::current().into();

    assert!(ActorSystem::new(backend()).is_ok());
    assert!(ActorSystem::with_name("valid-name", backend()).is_ok());

    assert!(ActorSystem::with_name("/", backend()).is_err());
    assert!(ActorSystem::with_name("*", backend()).is_err());
    assert!(ActorSystem::with_name("/a/b/c", backend()).is_err());
    assert!(ActorSystem::with_name("@", backend()).is_err());
    assert!(ActorSystem::with_name("#", backend()).is_err());
    assert!(ActorSystem::with_name("abc*", backend()).is_err());
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

#[tokio::test]
async fn system_shutdown() {
    let backend = Handle::current().into();
    let sys = ActorSystem::new(backend).unwrap();

    let _ = sys
        .actor_of_args::<ShutdownTest, _>("test-actor-1", 1)
        .unwrap();

    sys.shutdown().await;
}

#[tokio::test]
async fn system_builder() {
    use Handle;

    let backend = Handle::current().into();
    let sys = SystemBuilder::new().exec(backend).create().unwrap();
    sys.shutdown().await;

    let backend = Handle::current().into();
    let sys = SystemBuilder::new()
        .name("my-sys")
        .exec(backend)
        .create()
        .unwrap();
    sys.shutdown().await;
}
