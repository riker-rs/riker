use std::{
    fmt,
    ops::Deref,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use chrono::prelude::*;
use config::Config;
use futures::{
    channel::oneshot,
    executor::{ThreadPool, ThreadPoolBuilder},
    future::RemoteHandle,
    task::{SpawnError, SpawnExt},
    Future,
};
use log::{debug, Level};
use rand;
use uuid::Uuid;

use crate::{
    actor::*,
    kernel::provider::{create_root, Provider},
    load_config,
    system::logger::*,
    system::timer::*,
    system::{ActorTerminated, SystemCmd, SystemError, SystemEvent, SystemMsg},
    validate::{validate_name, InvalidPath},
    AnyMessage, Message,
};

// 0. error results on any
// 1. visibility

pub struct ProtoSystem {
    id: Uuid,
    name: String,
    pub host: Arc<String>,
    config: Config,
    pub(crate) sys_settings: SystemSettings,
    started_at: DateTime<Utc>,
}

pub struct SystemBuilder {
    name: Option<String>,
    cfg: Option<Config>,
    log: Option<BoxActorProd<LogActor>>,
    exec: Option<ThreadPool>,
}

impl SystemBuilder {
    pub fn new() -> Self {
        SystemBuilder {
            name: None,
            cfg: None,
            log: None,
            exec: None,
        }
    }

    pub async fn create(self) -> Result<ActorSystem, SystemError> {
        let cfg = self.cfg.unwrap_or(load_config());
        let exec = self.exec.unwrap_or(default_exec(&cfg));
        let log = self.log.unwrap_or(default_log(&cfg));

        ActorSystem::create(self.name.as_ref().unwrap(), exec, log, cfg).await
    }

    pub fn name(self, name: &str) -> Self {
        SystemBuilder {
            name: Some(name.to_string()),
            ..self
        }
    }

    pub fn cfg(self, cfg: Config) -> Self {
        SystemBuilder {
            cfg: Some(cfg),
            ..self
        }
    }

    pub fn exec(self, exec: ThreadPool) -> Self {
        SystemBuilder {
            exec: Some(exec),
            ..self
        }
    }
}

/// The actor runtime and common services coordinator
///
/// The `ActorSystem` provides a runtime on which actors are executed.
/// It also provides common services such as channels, persistence
/// and scheduling. The `ActorSystem` is the heart of a Riker application,
/// starting serveral threads when it is created. Create only one instance
/// of `ActorSystem` per application.
#[allow(dead_code)]
#[derive(Clone)]
pub struct ActorSystem {
    proto: Arc<ProtoSystem>,
    sys_actors: Option<SysActors>,
    log: Option<Logger>,
    debug: bool,
    pub exec: ThreadPool,
    pub timer: TimerRef,
    pub sys_channels: Option<SysChannels>,
    pub(crate) provider: Provider,
}

impl ActorSystem {
    /// Create a new `ActorSystem` instance
    ///
    /// Requires a type that implements the `Model` trait.
    pub async fn new() -> Result<ActorSystem, SystemError> {
        let cfg = load_config();
        let exec = default_exec(&cfg);
        let log = default_log(&cfg);

        ActorSystem::create("riker", exec, log, cfg).await
    }

    /// Create a new `ActorSystem` instance with provided name
    ///
    /// Requires a type that implements the `Model` trait.
    pub async fn with_name(name: &str) -> Result<ActorSystem, SystemError> {
        let cfg = load_config();
        let exec = default_exec(&cfg);
        let log = default_log(&cfg);

        ActorSystem::create(name, exec, log, cfg).await
    }

    /// Create a new `ActorSystem` instance bypassing default config behavior
    pub async fn with_config(name: &str, cfg: Config) -> Result<ActorSystem, SystemError> {
        let exec = default_exec(&cfg);
        let log = default_log(&cfg);

        ActorSystem::create(name, exec, log, cfg).await
    }

    async fn create(
        name: &str,
        exec: ThreadPool,
        log: BoxActorProd<LogActor>,
        cfg: Config,
    ) -> Result<ActorSystem, SystemError> {
        validate_name(name).map_err(|_| SystemError::InvalidName(name.into()))?;
        // Process Configuration
        let debug = cfg.get_bool("debug").unwrap();

        // Until the logger has started, use println
        if debug {
            println!("Starting actor system: System[{}]", name);
        }

        let prov = Provider::new();
        let timer = BasicTimer::start(&cfg);

        // 1. create proto system
        let proto = ProtoSystem {
            id: Uuid::new_v4(),
            name: name.to_string(),
            host: Arc::new("localhost".to_string()),
            config: cfg.clone(),
            sys_settings: SystemSettings::from(&cfg),
            started_at: Utc::now(),
        };

        // 2. create uninitialized system
        let mut sys = ActorSystem {
            proto: Arc::new(proto),
            debug,
            exec,
            log: None,
            // event_store: None,
            timer,
            sys_channels: None,
            sys_actors: None,
            provider: prov.clone(),
        };

        // 3. create initial actor hierarchy
        let sys_actors = create_root(&sys).await;
        sys.sys_actors = Some(sys_actors);

        // 4. start logger
        sys.log = Some(logger(&prov, &sys, &cfg, log).await?);

        // 5. start system channels
        sys.sys_channels = Some(sys_channels(&prov, &sys).await?);

        // 6. start dead letter logger
        let props = DeadLetterLogger::props(sys.dead_letters());
        let _dl_logger = sys_actor_of(&prov, &sys, props, "dl_logger").await?;

        sys.complete_start();

        debug!("Actor system [{}] [{}] started", sys.id(), name);

        Ok(sys)
    }

    fn complete_start(&self) {
        self.sys_actors.as_ref().unwrap().user.sys_init(self);
    }

    /// Returns the system start date
    pub fn start_date(&self) -> &DateTime<Utc> {
        &self.proto.started_at
    }

    /// Returns the number of seconds since the system started
    pub fn uptime(&self) -> u64 {
        let now = Utc::now();
        now.time()
            .signed_duration_since(self.start_date().time())
            .num_seconds() as u64
    }

    /// Returns the hostname used when the system started
    ///
    /// The host is used in actor addressing.
    ///
    /// Currently not used, but will be once system clustering is introduced.
    pub fn host(&self) -> Arc<String> {
        self.proto.host.clone()
    }

    /// Returns the UUID assigned to the system
    pub fn id(&self) -> Uuid {
        self.proto.id.clone()
    }

    /// Returns the name of the system
    pub fn name(&self) -> String {
        self.proto.name.clone()
    }

    pub fn print_tree(&self) {
        fn print_node(sys: &ActorSystem, node: BasicActorRef, indent: &str) {
            if node.is_root() {
                println!("{}", sys.name());

                for actor in node.children() {
                    print_node(sys, actor, "");
                }
            } else {
                println!("{}└─ {}", indent, node.name());

                for actor in node.children() {
                    print_node(sys, actor, &(indent.to_string() + "   "));
                }
            }
        }

        let root = self.sys_actors.as_ref().unwrap().root.clone();
        print_node(self, root, "");
    }

    /// Returns the system root's actor reference
    #[allow(dead_code)]
    fn root(&self) -> &BasicActorRef {
        &self.sys_actors.as_ref().unwrap().root
    }

    /// Returns the user root actor reference
    pub fn user_root(&self) -> &BasicActorRef {
        &self.sys_actors.as_ref().unwrap().user
    }

    /// Returns the system root actor reference
    pub fn sys_root(&self) -> &BasicActorRef {
        &self.sys_actors.as_ref().unwrap().sysm
    }

    /// Reutrns the temp root actor reference
    pub fn temp_root(&self) -> &BasicActorRef {
        &self.sys_actors.as_ref().unwrap().temp
    }

    /// Returns a reference to the system events channel
    pub fn sys_events(&self) -> &ActorRef<ChannelMsg<SystemEvent>> {
        &self.sys_channels.as_ref().unwrap().sys_events
    }

    /// Returns a reference to the dead letters channel
    pub fn dead_letters(&self) -> &ActorRef<DLChannelMsg> {
        &self.sys_channels.as_ref().unwrap().dead_letters
    }

    pub fn publish_event(&self, evt: SystemEvent) {
        let topic = Topic::from(&evt);
        self.sys_events().tell(Publish { topic, msg: evt }, None);
    }

    /// Returns the `Config` used by the system
    pub fn config(&self) -> Config {
        self.proto.config.clone()
    }

    pub(crate) fn sys_settings(&self) -> &SystemSettings {
        &self.proto.sys_settings
    }

    /// Create an actor under the system root
    pub async fn sys_actor_of<A>(
        &self,
        props: BoxActorProd<A>,
        name: &str,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        self.provider
            .create_actor(props, name, &self.sys_root(), self)
            .await
    }

    /// Shutdown the actor system
    ///
    /// Attempts a graceful shutdown of the system and all actors.
    /// Actors will receive a stop message, executing `actor.post_stop`.
    ///
    /// Does not block. Returns a future which is completed when all
    /// actors have successfully stopped.
    pub async fn shutdown(&self) -> Shutdown {
        let (tx, rx) = oneshot::channel::<()>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let props = Props::new_args(ShutdownActor::new, tx);
        self.tmp_actor_of(props).await.unwrap();

        rx
    }
}

unsafe impl Send for ActorSystem {}
unsafe impl Sync for ActorSystem {}

#[async_trait]
impl ActorRefFactory for ActorSystem {
    async fn actor_of<A>(
        &self,
        props: BoxActorProd<A>,
        name: &str,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        self.provider
            .create_actor(props, name, &self.user_root(), self)
            .await
    }

    async fn stop<A>(&self, actor: A)
    where
        A: ActorReference
    {
        actor.sys_tell(SystemCmd::Stop.into()).await;
    }
}

#[async_trait]
impl TmpActorRefFactory for ActorSystem {
    async fn tmp_actor_of<A>(&self, props: BoxActorProd<A>) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        let name = format!("{}", rand::random::<u64>());
        self.provider
            .create_actor(props, &name, &self.temp_root(), self)
            .await
    }
}

impl ActorSelectionFactory for ActorSystem {
    fn select(&self, path: &str) -> Result<ActorSelection, InvalidPath> {
        let anchor = self.user_root();
        let (anchor, path_str) = if path.starts_with("/") {
            let anchor = self.user_root();
            let anchor_path = format!("{}/", anchor.path().deref().clone());
            let path = path.to_string().replace(&anchor_path, "");

            (anchor, path)
        } else {
            (anchor, path.to_string())
        };

        ActorSelection::new(
            anchor.clone(),
            // self.dead_letters(),
            path_str,
        )
    }
}

// futures::task::Spawn::spawn requires &mut self so
// we'll create a wrapper trait that requires only &self.
pub trait Run {
    fn run<Fut>(&self, future: Fut) -> Result<RemoteHandle<<Fut as Future>::Output>, SpawnError>
    where
        Fut: Future + Send + 'static,
        <Fut as Future>::Output: Send;
}

impl Run for ActorSystem {
    fn run<Fut>(&self, future: Fut) -> Result<RemoteHandle<<Fut as Future>::Output>, SpawnError>
    where
        Fut: Future + Send + 'static,
        <Fut as Future>::Output: Send,
    {
        let mut exec = self.exec.clone();
        exec.spawn_with_handle(future)
    }
}

impl fmt::Debug for ActorSystem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ActorSystem[Name: {}, Start Time: {}, Uptime: {} seconds]",
            self.name(),
            self.start_date(),
            self.uptime()
        )
    }
}

impl Timer for ActorSystem {
    fn schedule<T, M>(
        &self,
        initial_delay: Duration,
        interval: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message,
    {
        let id = Uuid::new_v4();
        let msg: M = msg.into();

        let job = RepeatJob {
            id: id.clone(),
            send_at: SystemTime::now() + initial_delay,
            interval: interval,
            receiver: receiver.into(),
            sender: sender,
            msg: AnyMessage::new(msg, false),
        };

        let _ = self.timer.send(Job::Repeat(job));
        id
    }

    fn schedule_once<T, M>(
        &self,
        delay: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message,
    {
        let id = Uuid::new_v4();
        let msg: M = msg.into();

        let job = OnceJob {
            id: id.clone(),
            send_at: SystemTime::now() + delay,
            receiver: receiver.into(),
            sender,
            msg: AnyMessage::new(msg, true),
        };

        let _ = self.timer.send(Job::Once(job));
        id
    }

    fn schedule_at_time<T, M>(
        &self,
        time: DateTime<Utc>,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message,
    {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(time.timestamp() as u64);

        let id = Uuid::new_v4();
        let msg: M = msg.into();

        let job = OnceJob {
            id: id.clone(),
            send_at: time,
            receiver: receiver.into(),
            sender: sender,
            msg: AnyMessage::new(msg, true),
        };

        let _ = self.timer.send(Job::Once(job));
        id
    }

    fn cancel_schedule(&self, id: Uuid) {
        let _ = self.timer.send(Job::Cancel(id));
    }
}

// helper functions

async fn sys_actor_of<A>(
    prov: &Provider,
    sys: &ActorSystem,
    props: BoxActorProd<A>,
    name: &str,
) -> Result<ActorRef<A::Msg>, SystemError>
where
    A: Actor,
{
    prov.create_actor(props, name, &sys.sys_root(), sys)
        .await
        .map_err(|_| SystemError::ModuleFailed(name.into()))
}

async fn logger(
    prov: &Provider,
    sys: &ActorSystem,
    cfg: &Config,
    props: BoxActorProd<LogActor>,
) -> Result<Logger, SystemError> {
    let logger = sys_actor_of(prov, sys, props, "logger").await?;

    let level = cfg
        .get_str("log.level")
        .map(|l| Level::from_str(&l))
        .unwrap()
        .unwrap();
    Ok(Logger::init(level, logger))
}

async fn sys_channels(prov: &Provider, sys: &ActorSystem) -> Result<SysChannels, SystemError> {
    let props = Props::new(EventsChannel::new);
    let sys_events = sys_actor_of(prov, sys, props, "sys_events").await?;

    let props = Props::new(Channel::<DeadLetter>::new);
    let dead_letters = sys_actor_of(prov, sys, props, "dead_letters").await?;

    // subscribe the dead_letters channel to actor terminated events
    // so that any future subscribed actors that terminate are automatically
    // unsubscribed from the dead_letters channel
    // let msg = ChannelMsg::Subscribe(SysTopic::ActorTerminated.into(), dl.clone());
    // es.tell(msg, None);

    Ok(SysChannels {
        sys_events,
        dead_letters,
    })
}

pub struct SystemSettings {
    pub msg_process_limit: u32,
}

impl<'a> From<&'a Config> for SystemSettings {
    fn from(config: &Config) -> Self {
        SystemSettings {
            msg_process_limit: config.get_int("mailbox.msg_process_limit").unwrap() as u32,
        }
    }
}

struct ThreadPoolConfig {
    pool_size: usize,
}

impl<'a> From<&'a Config> for ThreadPoolConfig {
    fn from(config: &Config) -> Self {
        ThreadPoolConfig {
            pool_size: config.get_int("dispatcher.pool_size").unwrap() as usize,
        }
    }
}

fn default_exec(cfg: &Config) -> ThreadPool {
    let exec_cfg = ThreadPoolConfig::from(cfg);
    ThreadPoolBuilder::new()
        .pool_size(exec_cfg.pool_size)
        .name_prefix("pool-thread-#")
        .create()
        .unwrap()
}

fn default_log(cfg: &Config) -> BoxActorProd<LogActor> {
    let cfg = LoggerConfig::from(cfg);
    SimpleLogger::props(cfg)
}

#[derive(Clone)]
pub struct SysActors {
    pub root: BasicActorRef,
    pub user: BasicActorRef,
    pub sysm: BasicActorRef,
    pub temp: BasicActorRef,
}

#[derive(Clone)]
pub struct SysChannels {
    pub sys_events: ActorRef<ChannelMsg<SystemEvent>>,
    pub dead_letters: ActorRef<DLChannelMsg>,
}

pub type Shutdown = oneshot::Receiver<()>;

#[derive(Clone)]
struct ShutdownActor {
    tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl ShutdownActor {
    fn new(tx: Arc<Mutex<Option<oneshot::Sender<()>>>>) -> Self {
        ShutdownActor { tx }
    }
}

#[async_trait]
impl Actor for ShutdownActor {
    type Msg = SystemEvent;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Subscribe {
            topic: SysTopic::ActorTerminated.into(),
            actor: Box::new(ctx.myself.clone()),
        };
        ctx.system.sys_events().tell(sub, None);

        // todo this is prone to failing since there is no
        // confirmation that ShutdownActor has subscribed to
        // the ActorTerminated events yet.
        // It may be that the user root actor is Sterminated
        // before the subscription is complete.

        // std::thread::sleep_ms(1000);
        // send stop to all /user children
        ctx.system.stop(ctx.system.user_root());
    }

    async fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
        if let SystemMsg::Event(evt) = msg {
            if let SystemEvent::ActorTerminated(terminated) = evt {
                self.receive(ctx, terminated, sender);
            }
        }
    }

    async fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<BasicActorRef>) {}
}

#[async_trait]
impl Receive<ActorTerminated> for ShutdownActor {
    type Msg = SystemEvent;

    async fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: ActorTerminated,
        _sender: Option<BasicActorRef>,
    ) {
        if &msg.actor == ctx.system.user_root() {
            if let Ok(ref mut tx) = self.tx.lock() {
                if let Some(tx) = tx.take() {
                    tx.send(()).unwrap();
                }
            }
        }
    }
}
