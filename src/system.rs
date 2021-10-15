pub(crate) mod logger;
pub(crate) mod timer;

use std::fmt;

use crate::actor::BasicActorRef;

// Public API (plus the pub data types in this file)
pub use self::timer::{BasicTimer, ScheduleId, Timer};

#[derive(Clone, Debug)]
pub enum SystemMsg {
    ActorInit,
    Command(SystemCmd),
    Event(SystemEvent),
    Failed(BasicActorRef),
}

#[derive(Clone, Debug)]
pub enum SystemCmd {
    Stop,
    Restart,
}

impl Into<SystemMsg> for SystemCmd {
    fn into(self) -> SystemMsg {
        SystemMsg::Command(self)
    }
}

#[derive(Clone, Debug)]
pub enum SystemEvent {
    /// An actor was terminated
    ActorCreated(ActorCreated),

    /// An actor was restarted
    ActorRestarted(ActorRestarted),

    /// An actor was started
    ActorTerminated(ActorTerminated),
}

impl Into<SystemMsg> for SystemEvent {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(self)
    }
}

#[derive(Clone, Debug)]
pub struct ActorCreated {
    pub actor: BasicActorRef,
}

#[derive(Clone, Debug)]
pub struct ActorRestarted {
    pub actor: BasicActorRef,
}

#[derive(Clone, Debug)]
pub struct ActorTerminated {
    pub actor: BasicActorRef,
}

impl Into<SystemEvent> for ActorCreated {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorCreated(self)
    }
}

impl Into<SystemEvent> for ActorRestarted {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorRestarted(self)
    }
}

impl Into<SystemEvent> for ActorTerminated {
    fn into(self) -> SystemEvent {
        SystemEvent::ActorTerminated(self)
    }
}

impl Into<SystemMsg> for ActorCreated {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorCreated(self))
    }
}

impl Into<SystemMsg> for ActorRestarted {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorRestarted(self))
    }
}

impl Into<SystemMsg> for ActorTerminated {
    fn into(self) -> SystemMsg {
        SystemMsg::Event(SystemEvent::ActorTerminated(self))
    }
}

#[derive(Clone, Debug)]
pub enum SystemEventType {
    ActorTerminated,
    ActorRestarted,
    ActorCreated,
}

pub enum SystemError {
    ModuleFailed(String),
    InvalidName(String),
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SystemError::ModuleFailed(ref m) => f.write_str(&format!(
                "Failed to create actor system. Cause: Sub module failed to start ({})",
                m
            )),
            SystemError::InvalidName(ref name) => f.write_str(&format!(
                "Failed to create actor system. Cause: Invalid actor system name ({})",
                name
            )),
        }
    }
}

impl fmt::Debug for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}
use std::{
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use uuid::Uuid;

use crate::{
    actor::{props::ActorFactory, *},
    kernel::provider::{create_root, Provider},
    load_config,
    system::logger::*,
    system::timer::*,
    validate::validate_name,
    AnyMessage, Config, Message,
};
use slog::Logger;

// 0. error results on any
// 1. visibility

pub struct ProtoSystem {
    id: Uuid,
    name: String,
    pub host: Arc<str>,
    config: Config,
    pub(crate) sys_settings: SystemSettings,
    started_at: SystemTime,
    started_at_moment: Instant,
}

#[derive(Default)]
pub struct SystemBuilder {
    name: Option<String>,
    cfg: Option<Config>,
    log: Option<Logger>,
}

impl SystemBuilder {
    pub fn new() -> Self {
        SystemBuilder::default()
    }

    pub fn create(self) -> Result<ActorSystem, SystemError> {
        let name = self
            .name
            .unwrap_or_else(|| "tezedge-actor-system".to_string());
        let cfg = self.cfg.unwrap_or_else(load_config);
        let log = self.log.unwrap_or_else(|| default_log(&cfg));

        ActorSystem::create(name.as_ref(), log, cfg)
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

    pub fn log(self, log: Logger) -> Self {
        SystemBuilder {
            log: Some(log),
            ..self
        }
    }
}

/// The actor runtime and common services coordinator
///
/// The `ActorSystem` provides a runtime on which actors are executed.
/// It also provides common services such as channels and scheduling.
/// The `ActorSystem` is the heart of a Riker application,
/// starting several threads when it is created. Create only one instance
/// of `ActorSystem` per application.
#[derive(Clone)]
pub struct ActorSystem {
    proto: Arc<ProtoSystem>,
    sys_actors: Option<SysActors>,
    log: Logger,
    debug: bool,
    pub timer: Arc<Mutex<TimerRef>>,
    sys_channels: Option<SysChannels>,
    temp_storage: Arc<Mutex<Option<(SysActors, SysChannels)>>>,
    pub(super) provider: Provider,
    shutdown_rx: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

impl ActorSystem {
    /// Create a new `ActorSystem` instance
    ///
    /// Requires a type that implements the `Model` trait.
    pub fn new() -> Result<ActorSystem, SystemError> {
        let cfg = load_config();
        let log = default_log(&cfg);

        ActorSystem::create("tezedge-actor-system", log, cfg)
    }

    /// Create a new `ActorSystem` instance with provided name
    ///
    /// Requires a type that implements the `Model` trait.
    pub fn with_name(name: &str) -> Result<ActorSystem, SystemError> {
        let cfg = load_config();
        let log = default_log(&cfg);

        ActorSystem::create(name, log, cfg)
    }

    /// Create a new `ActorSystem` instance bypassing default config behavior
    pub fn with_config(name: &str, cfg: Config) -> Result<ActorSystem, SystemError> {
        let log = default_log(&cfg);

        ActorSystem::create(name, log, cfg)
    }

    fn create(name: &str, log: Logger, cfg: Config) -> Result<ActorSystem, SystemError> {
        validate_name(name).map_err(|_| SystemError::InvalidName(name.into()))?;
        // Process Configuration
        let debug = cfg.debug;

        // Until the logger has started, use println
        if debug {
            slog::debug!(log, "Starting actor system: System[{}]", name);
        }

        let prov = Provider::new(log.clone());
        let timer = BasicTimer::start(&cfg);

        // 1. create proto system
        let proto = ProtoSystem {
            id: Uuid::new_v4(),
            name: name.to_string(),
            host: Arc::from("localhost"),
            config: cfg.clone(),
            sys_settings: SystemSettings {
                msg_process_limit: cfg.mailbox.msg_process_limit,
            },
            started_at: SystemTime::now(),
            started_at_moment: Instant::now(),
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        // 2. create uninitialized system
        let mut sys = ActorSystem {
            proto: Arc::new(proto),
            debug,
            log,
            // event_store: None,
            timer: Arc::new(Mutex::new(timer)),
            sys_channels: None,
            sys_actors: None,
            temp_storage: Arc::new(Mutex::new(None)),
            provider: prov.clone(),
            shutdown_rx: Arc::new(Mutex::new(Some(shutdown_rx))),
        };

        // 3. create initial actor hierarchy
        let sys_actors = create_root(&sys, shutdown_tx);
        sys.sys_actors = Some(sys_actors.clone());

        // 4. start system channels
        let sys_channels = sys_channels(&prov, &sys)?;
        sys.sys_channels = Some(sys_channels.clone());

        // 5. start dead letter logger
        let _dl_logger = sys_actor_of_args::<DeadLetterLogger, _>(
            &prov,
            &sys,
            "dl_logger",
            (sys.dead_letters().clone(), sys.log()),
        )?;

        *sys.temp_storage.lock().unwrap() = Some((sys_actors, sys_channels));
        sys.sys_actors.as_ref().unwrap().user.sys_init();

        slog::debug!(sys.log, "Actor system [{}] [{}] started", sys.id(), name);

        Ok(sys)
    }

    pub(crate) fn complete_start(&mut self) {
        let (sys_actors, sys_channels) = self.temp_storage.lock().unwrap().clone().unwrap();
        self.sys_actors = Some(sys_actors);
        self.sys_channels = Some(sys_channels);
    }

    /// Returns the system start moment
    pub fn start_date(&self) -> SystemTime {
        self.proto.started_at
    }

    /// Returns the number of seconds since the system started
    pub fn uptime(&self) -> u64 {
        let now = Instant::now();
        now.duration_since(self.proto.started_at_moment).as_secs() as u64
    }

    /// Returns the hostname used when the system started
    ///
    /// The host is used in actor addressing.
    ///
    /// Currently not used, but will be once system clustering is introduced.
    pub fn host(&self) -> Arc<str> {
        self.proto.host.clone()
    }

    /// Returns the UUID assigned to the system
    pub fn id(&self) -> Uuid {
        self.proto.id
    }

    /// Returns the name of the system
    pub fn name(&self) -> String {
        self.proto.name.clone()
    }

    pub fn print_tree(&self) -> Vec<String> {
        fn print_node(
            sys: &ActorSystem,
            node: &BasicActorRef,
            indent: &str,
            log: &mut Vec<String>,
        ) {
            if node.is_root() {
                log.push(sys.name());

                for actor in node.children() {
                    print_node(sys, &actor, "", log);
                }
            } else {
                log.push(format!("{}└─ {}", indent, node.name()));

                for actor in node.children() {
                    print_node(sys, &actor, &(indent.to_string() + "   "), log);
                }
            }
        }

        let mut log: Vec<String> = Vec::new();
        let root = self.root();
        print_node(self, root, "", &mut log);
        log
    }

    /// Returns the system root's actor reference
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
    pub fn config(&self) -> &Config {
        &self.proto.config
    }

    pub(crate) fn sys_settings(&self) -> &SystemSettings {
        &self.proto.sys_settings
    }

    #[inline]
    pub fn log(&self) -> Logger {
        self.log.clone()
    }

    /// Shutdown the actor system
    ///
    /// Attempts a graceful shutdown of the system and all actors.
    /// Actors will receive a stop message, executing `actor.post_stop`.
    ///
    /// Block until all actors have successfully stopped.
    pub fn shutdown(&self) {
        self.stop(self.user_root());
        let _ = self
            .shutdown_rx
            .lock()
            .expect("poisoned")
            .take()
            .expect("shutdown was already called")
            .recv();
    }
}

impl ActorRefFactory for ActorSystem {
    fn actor_of_props<A>(
        &self,
        name: &str,
        props: BoxActorProd<A>,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        self.provider
            .create_actor(props, name, self.user_root(), self)
    }

    fn actor_of<A>(&self, name: &str) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
    where
        A: ActorFactory,
    {
        self.provider
            .create_actor(Props::new::<A>(), name, self.user_root(), self)
    }

    fn actor_of_args<A, Args>(
        &self,
        name: &str,
        args: Args,
    ) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
    where
        Args: ActorArgs,
        A: ActorFactoryArgs<Args>,
    {
        self.provider
            .create_actor(Props::new_args::<A, _>(args), name, self.user_root(), self)
    }

    fn stop(&self, actor: impl ActorReference) {
        actor.sys_tell(SystemCmd::Stop.into());
    }
}

impl ActorRefFactory for &ActorSystem {
    fn actor_of_props<A>(
        &self,
        name: &str,
        props: BoxActorProd<A>,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        self.provider
            .create_actor(props, name, self.user_root(), self)
    }

    fn actor_of<A>(&self, name: &str) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
    where
        A: ActorFactory,
    {
        self.provider
            .create_actor(Props::new::<A>(), name, self.user_root(), self)
    }

    fn actor_of_args<A, Args>(
        &self,
        name: &str,
        args: Args,
    ) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
    where
        Args: ActorArgs,
        A: ActorFactoryArgs<Args>,
    {
        self.provider
            .create_actor(Props::new_args::<A, _>(args), name, self.user_root(), self)
    }

    fn stop(&self, actor: impl ActorReference) {
        actor.sys_tell(SystemCmd::Stop.into());
    }
}

impl fmt::Debug for ActorSystem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ActorSystem[Name: {}, Start Time: {:?}, Uptime: {} seconds]",
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
    ) -> ScheduleId
    where
        T: Message + Into<M>,
        M: Message,
    {
        let id = Uuid::new_v4();
        let msg: M = msg.into();

        let job = RepeatJob {
            id,
            send_at: Instant::now() + initial_delay,
            interval,
            receiver: receiver.into(),
            sender,
            msg: AnyMessage::new(msg, false),
        };

        let _ = self.timer.lock().unwrap().send(Job::Repeat(job));
        id
    }

    fn schedule_once<T, M>(
        &self,
        delay: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> ScheduleId
    where
        T: Message + Into<M>,
        M: Message,
    {
        let id = Uuid::new_v4();
        let msg: M = msg.into();

        let job = OnceJob {
            id,
            send_at: Instant::now() + delay,
            receiver: receiver.into(),
            sender,
            msg: AnyMessage::new(msg, true),
        };

        let _ = self.timer.lock().unwrap().send(Job::Once(job));
        id
    }

    fn cancel_schedule(&self, id: Uuid) {
        let _ = self.timer.lock().unwrap().send(Job::Cancel(id));
    }
}

fn sys_actor_of<A>(
    prov: &Provider,
    sys: &ActorSystem,
    name: &str,
) -> Result<ActorRef<<A as Actor>::Msg>, SystemError>
where
    A: ActorFactory,
{
    prov.create_actor(Props::new::<A>(), name, sys.sys_root(), sys)
        .map_err(|_| SystemError::ModuleFailed(name.into()))
}

#[allow(dead_code)]
fn sys_actor_of_args<A, Args>(
    prov: &Provider,
    sys: &ActorSystem,
    name: &str,
    args: Args,
) -> Result<ActorRef<<A as Actor>::Msg>, SystemError>
where
    Args: ActorArgs,
    A: ActorFactoryArgs<Args>,
{
    prov.create_actor(Props::new_args::<A, _>(args), name, sys.sys_root(), sys)
        .map_err(|_| SystemError::ModuleFailed(name.into()))
}

fn sys_channels(prov: &Provider, sys: &ActorSystem) -> Result<SysChannels, SystemError> {
    let sys_events = sys_actor_of::<EventsChannel>(prov, sys, "sys_events")?;
    let dead_letters = sys_actor_of::<Channel<DeadLetter>>(prov, sys, "dead_letters")?;

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

#[derive(Clone)]
pub struct SysActors {
    pub root: BasicActorRef,
    pub user: BasicActorRef,
    pub sysm: BasicActorRef,
}

#[derive(Clone)]
pub struct SysChannels {
    sys_events: ActorRef<ChannelMsg<SystemEvent>>,
    dead_letters: ActorRef<DLChannelMsg>,
}
