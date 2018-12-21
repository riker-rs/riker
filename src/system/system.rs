use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, Duration};

use chrono::prelude::*;
use config::Config;
use rand;
use uuid::Uuid;
use futures::channel::oneshot::{channel, Sender, Receiver, Canceled};
use futures::{Future, Poll, task};
use log::{log, debug, Level};

use crate::futures_util::DispatchHandle;
use crate::model::Model;
use crate::protocol::{Message, ActorMsg, SystemMsg, ChannelMsg, ActorCmd, SystemEvent, IOMsg};
use crate::ExecutionContext;

use crate::system::timer::{Timer, TimerFactory, Job, OnceJob, RepeatJob};
use crate::system::persist::EsManager;
use crate::system::logger::{Logger, LoggerProps, DeadLetterProps};
use crate::system::{Io, IoManagerProps, SystemError};
use crate::kernel::{Kernel, KernelRef, KernelMsg, SysActors};
use crate::actor::*;
use crate::load_config;
use crate::validate::{validate_name, InvalidPath};

pub struct ProtoSystem {
    id: Uuid,
    name: String,
    pub host: Arc<String>,
    config: Config,
    started_at: DateTime<Utc>,
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
pub struct ActorSystem<Msg: Message> {
    pub proto: Arc<ProtoSystem>,
    pub kernel: Option<KernelRef<Msg>>,
    sys_actors: Option<SysActors<Msg>>,
    default_stream: Option<ActorRef<Msg>>,
    logger: Option<Logger<Msg>>,
    pub event_store: Option<ActorRef<Msg>>,
    pub sys_channels: Option<SysChannels<Msg>>,
    io_manager: Option<ActorRef<Msg>>,
    debug: bool,
}

impl<Msg: Message> ActorSystem<Msg> {
    /// Create a new `ActorSystem` instance
    ///
    /// Requires a type that implements the `Model` trait.
    pub fn new<Mdl>(model: &Mdl)
            -> Result<ActorSystem<Msg>, SystemError>
        where Mdl: Model<Msg=Msg>
    {
        ActorSystem::create(model, "riker", load_config())
    }

    /// Create a new `ActorSystem` instance with provided name
    ///
    /// Requires a type that implements the `Model` trait.
    pub fn with_name<Mdl>(model: &Mdl, name: &str)
            -> Result<ActorSystem<Msg>, SystemError>
        where Mdl: Model<Msg=Msg>
    {
        ActorSystem::create(model, name, load_config())
    }

    /// Create a new `ActorSystem` instance bypassing default config behavior
    ///
    /// Requires a type that implements the `Model` trait.
    pub fn with_config<Mdl>(model: &Mdl, name: &str, cfg: Config)
            -> Result<ActorSystem<Msg>, SystemError>
        where Mdl: Model<Msg=Msg>
    {
        ActorSystem::create(model, name, cfg)
    }

    fn create<Mdl>(_: &Mdl, name: &str, config: Config)
            -> Result<ActorSystem<Msg>, SystemError>
        where Mdl: Model<Msg=Msg>
    {
        validate_name(name)
            .map_err(|_| SystemError::InvalidName(name.into()))?;
        
        // Process Configuration
        let debug = config.get_bool("debug").unwrap();

        // Until the logger has started, use println
        if debug {
            println!("Starting actor system: System[{}]", name);
        }

        let proto = ProtoSystem {
            id: Uuid::new_v4(),
            name: name.to_string(),
            host: Arc::new("localhost".to_string()),
            config: config.clone(),
            started_at: Utc::now(),
        };

        let mut system = ActorSystem::<Mdl::Msg> {
            proto: Arc::new(proto),
            debug,
            kernel: None,
            logger: None,
            default_stream: None,
            event_store: None,
            sys_channels: None,
            sys_actors: None,
            io_manager: None,
        };

        // start timer
        let timer = Mdl::Tmr::new(&config, debug);

        // start kernel
        let kernel_main: Kernel<Mdl::Msg, Mdl::Dis> = Kernel::new(&config);
        let (kernel, sys_actors) = kernel_main.start(&system, timer);
        system.kernel = Some(kernel.clone());
        system.sys_actors = Some(sys_actors);

        // start system channels
        let sys_channels = sys_channels(&system)?;
        system.sys_channels = Some(sys_channels.clone());
        
        // start logging
        let logger = logger(&system, &config, Mdl::Log::props(&config))?;
        system.logger = Some(logger);

        // start default stream
        let props = Props::new_args(Box::new(Channel::new), Some(system.event_stream().clone()));
        let ds = sys_actor_of(&system, props, "default_stream")?;
        system.default_stream = Some(ds);
        
        // start dead letter logger
        let props = Mdl::Ded::props(sys_channels.dead_letters.clone());
        let _dl_logger = sys_actor_of(&system, props, "dl_logger")?;
        
        // start event store
        let esm = sys_actor_of(&system, EsManager::<Mdl::Evs>::props(&config), "event_store")?;
        system.event_store = Some(esm);

        // start io manager
        let io = sys_actor_of(&system, Io::props(), "io_manager")?;
        system.io_manager = Some(io.clone());

        // start tcp
        if let Some(props) = Mdl::Tcp::props(&config) {
            io.tell(IOMsg::Manage("tcp".into(), props), None);
        }

        debug!("Actor system [{}] [{}] started", system.id(), name);

        kernel.kernel_tx.send(KernelMsg::Initialize(system.clone())).unwrap();

        Ok(system)
    }

    /// Shutdown the actor system
    ///
    /// Attempts a graceful shutdown of the system and all actors.
    /// Actors will receive a stop message, executing `actor.post_stop`.
    /// 
    /// Does not block. Returns a future which is completed when all
    /// actors have successfully stopped.
    pub fn shutdown(&self) -> Shutdown {
        let (tx, rx) = channel::<()>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let a = ShutdownActor {
            tx,
            kernel: self.kernel.clone().unwrap()
        };

        let props = Props::new_args(Box::new(ShutdownActor::actor), a);
        self.tmp_actor_of(props).unwrap();

        // send stop to all /user children
        self.stop(self.user_root());

        Shutdown {
            inner: rx,
        }
    }

    /// Returns the system start date
    pub fn start_date(&self) -> &DateTime<Utc> {
        &self.proto.started_at
    }

    /// Returns the number of seconds since the system started
    pub fn uptime(&self) -> u64 {
        let now = Utc::now();
        now.time().signed_duration_since(self
                                        .start_date()
                                        .time())
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
        // TODO refactor to be loop based since no tail recursion
        fn print_node<Msg: Message>(system: &ActorSystem<Msg>,
                                     node: ActorRef<Msg>, indent: &str) {
            if node.is_root() {
                println!("{}", system.name());

                for actor in node.children() {
                    print_node(system, actor, "");
                }
            } else {
                println!("{}└─ {}", indent, node.uri.name);

                for actor in node.children() {
                    print_node(system, actor, &(indent.to_string()+"   "));
                }
            }
        }

        let root = self.sys_actors.as_ref().unwrap().root.clone();
        print_node(self, root, "");
    }

    /// Returns the system root's actor reference 
    #[allow(dead_code)]
    fn root(&self) -> &ActorRef<Msg> {
        &self.sys_actors.as_ref().unwrap().root
    }

    /// Returns the user root actor reference
    pub fn user_root(&self) -> &ActorRef<Msg> {
        &self.sys_actors.as_ref().unwrap().user
    }

    /// Returns the system root actor reference
    pub fn system_root(&self) -> &ActorRef<Msg> {
        &self.sys_actors.as_ref().unwrap().sysm
    }

    /// Reutrns the temp root actor reference
    pub fn temp_root(&self) -> &ActorRef<Msg> {
        &self.sys_actors.as_ref().unwrap().temp
    }

    /// Returns a reference to the default stream channel
    pub fn default_stream(&self) -> &ActorRef<Msg> {
        &self.default_stream.as_ref().unwrap()
    }

    /// Returns a reference to the event stream channel
    pub fn event_stream(&self) -> &ActorRef<Msg> {
        &self.sys_channels.as_ref().unwrap().event_stream
    }

    /// Returns a reference to the dead letters channel
    pub fn dead_letters(&self) -> &ActorRef<Msg> {
        &self.sys_channels.as_ref().unwrap().dead_letters
    }

    // pub fn event_store(&self) -> Option<ActorRef<Msg>> {
    //     &self.event_store
    // }

    /// Returns a reference to the IO Manager
    pub fn io_manager(&self) -> &ActorRef<Msg> {
        &self.io_manager.as_ref().unwrap()
    }

    /// Returns the `Config` used by the system
    pub fn config(&self) -> Config {
        self.proto.config.clone()
    }

    /// Create an actor under the system root
    pub fn sys_actor_of(&self, props: BoxActorProd<Msg>, name: &str) -> Result<ActorRef<Msg>, CreateError> {
        self.kernel.as_ref().unwrap().create_actor(props, name, &self.system_root())
    }
}

fn sys_actor_of<Msg>(sys: &ActorSystem<Msg>,
                    props: BoxActorProd<Msg>,
                    name: &str)
                        -> Result<ActorRef<Msg>, SystemError>
    where Msg: Message
{
    sys.sys_actor_of(props, name)
        .map_err(|_| SystemError::ModuleFailed(name.into()))
}

fn sys_channels<Msg>(sys: &ActorSystem<Msg>)
                        -> Result<SysChannels<Msg>, SystemError>
    where Msg: Message
{
    let props = Props::new(Box::new(SystemChannel::new));
    let es = sys_actor_of(sys, props, "event_stream")?;

    let props = Props::new(Box::new(SystemChannel::new));
    let dl = sys_actor_of(sys, props, "dead_letters")?;
    let msg = ChannelMsg::Subscribe(SysTopic::ActorTerminated.into(), dl.clone());
    es.tell(msg, None);

    Ok(SysChannels {
        event_stream: es,
        dead_letters: dl,
    })
}

fn logger<Msg>(sys: &ActorSystem<Msg>,
                    cfg: &Config,
                    props: BoxActorProd<Msg>)
                        -> Result<Logger<Msg>, SystemError>
    where Msg: Message
{
    let logger = sys_actor_of(sys, props, "logger")?;

    let level = cfg.get_str("log.level").map(|l| Level::from_str(&l)).unwrap().unwrap();
    Ok(Logger::init(level, logger))
}

unsafe impl<Msg: Message> Send for ActorSystem<Msg> {}
unsafe impl<Msg: Message> Sync for ActorSystem<Msg> {}

impl<Msg> ActorRefFactory for ActorSystem<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn actor_of(&self,
                props: BoxActorProd<Self::Msg>,
                name: &str) -> Result<ActorRef<Msg>, CreateError> {
        self.kernel.as_ref().unwrap().create_actor(props, name, &self.user_root())
    }

    fn stop(&self, actor: &ActorRef<Self::Msg>) {
        let sys_msg = SystemMsg::ActorCmd(ActorCmd::Stop);
        actor.sys_tell(sys_msg, None);
    }
}

impl<Msg> TmpActorRefFactory for ActorSystem<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn tmp_actor_of(&self, props: BoxActorProd<Msg>)
                    -> Result<ActorRef<Msg>, CreateError> {
        let name = rand::random::<u64>();
        let name = format!("{}", name);
        self.kernel.as_ref().unwrap().create_actor(props, &name, &self.temp_root())
    }
}

impl<Msg> ActorSelectionFactory for ActorSystem<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn select(&self, path: &str)
                -> Result<ActorSelection<Msg>, InvalidPath> {
        ActorSelection::new(self.user_root(), path)
    }
}

impl<Msg> Timer for ActorSystem<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn schedule<T>(&self,
        initial_delay: Duration,
        interval: Duration,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T) -> Uuid
            where T: Into<ActorMsg<Self::Msg>>
    {
        
        let id = Uuid::new_v4();
        
        let job = RepeatJob {
            id: id.clone(),
            send_at: SystemTime::now() + initial_delay,
            interval: interval,
            receiver: receiver,
            sender: sender,
            msg: msg.into()
        };

        self.kernel.as_ref().unwrap().schedule(Job::Repeat(job));
        id
    }

    fn schedule_once<T>(&self,
        delay: Duration,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T) -> Uuid
            where T: Into<ActorMsg<Self::Msg>>
    {
        
        let id = Uuid::new_v4();

        let job = OnceJob {
            id: id.clone(),
            send_at: SystemTime::now() + delay,
            receiver: receiver,
            sender: sender,
            msg: msg.into()
        };

        self.kernel.as_ref().unwrap().schedule(Job::Once(job));
        id
    }

    fn schedule_at_time<T>(&self,
        time: DateTime<Utc>,
        receiver: ActorRef<Self::Msg>,
        sender: Option<ActorRef<Self::Msg>>,
        msg: T) -> Uuid
            where T: Into<ActorMsg<Self::Msg>>
    {
        let time = SystemTime::UNIX_EPOCH +
            Duration::from_secs(time.timestamp() as u64);
        
        let id = Uuid::new_v4();

        let job = OnceJob {
            id: id.clone(),
            send_at: time,
            receiver: receiver,
            sender: sender,
            msg: msg.into()
        };

        self.kernel.as_ref().unwrap().schedule(Job::Once(job));
        id
    }

    fn cancel_schedule(&self, id: Uuid) {
        self.kernel.as_ref().unwrap().schedule(Job::Cancel(id));
    }
}

impl<Msg> ExecutionContext for ActorSystem<Msg>
    where Msg: Message
{
    fn execute<F: Future>(&self, f: F) -> DispatchHandle<F::Item, F::Error>
        where F: Future + Send + 'static,
                F::Item: Send + 'static,
                F::Error: Send + 'static,
    {
        self.kernel.as_ref().unwrap().execute(f)
    }
}

impl<Msg> fmt::Debug for ActorSystem<Msg>
    where Msg: Message
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
                "ActorSystem[Name: {}, Start Time: {}, Uptime: {} seconds]",
                self.name(),
                self.start_date(),
                self.uptime())
    }
}

pub struct Shutdown {
    inner: Receiver<()>,
}

impl Future for Shutdown {
    type Item = ();
    type Error = Canceled;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        self.inner.poll(cx)
    }
}

#[derive(Clone)]
struct ShutdownActor<Msg: Message> {
    tx: Arc<Mutex<Option<Sender<()>>>>,
    kernel: KernelRef<Msg>,
}

impl<Msg: Message> ShutdownActor<Msg> {
    fn actor(a: Self) -> BoxActor<Msg> {
        Box::new(a)
    }

    fn check_and_complete(&self, user_guardian: &ActorRef<Msg>) -> bool {
        // After all children have stopped the future can be completed
        if user_guardian.children().count() == 0 {
            match self.tx.lock() {
                // TODO DOC it's possible that the last child has been removed
                // by the time that this executes, and tx is already None
                Ok(ref mut tx) if tx.is_some() => drop(tx.take().unwrap().send(())),
                _ => {}
            }

            // Signal the kernel to stop, resulting in the kernel thread to released
            self.kernel.stop_kernel();
            true
        } else {
            false
        }
    }
}

impl<Msg: Message> Actor for ShutdownActor<Msg> {
    type Msg = Msg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        if !self.check_and_complete(&ctx.system.user_root()) {
            let msg = ChannelMsg::Subscribe(SysTopic::ActorTerminated.into(), ctx.myself.clone());
            ctx.system.event_stream().tell(msg, None);
        }
    }

    fn system_receive(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, _: Option<ActorRef<Self::Msg>>) {
        if let SystemMsg::Event(evt) = msg {
            if let SystemEvent::ActorTerminated(_) = evt {
                let _ = self.check_and_complete(&ctx.system.user_root());
            }
        }   
    }

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
}


// #[cfg(test)]
// mod tests {
//     extern crate futures;

//     use super::*;

//     use test_mocks::{MockModel, get_test_config};
//     use config::Config;

//     #[derive(Debug, Clone]
//     struct TestMsg;
//     unsafe impl Send for TestMsg {}

//     #[test]
//     fn create_system_bad_name() {
//         let model = MockModel;

//         let mut cfg = Config::new();
//         cfg.set("am_actors.debug", true).unwrap();
//         let asys = ActorSystem::with_config(&model, "!%@#^!@#", &cfg);
//         assert!(asys.is_err());
//     }

//     #[test]
//     fn create_system_no_config_is_error() {
//         let model = MockModel;

//         let mut cfg = Config::new();
//         cfg.set("am_actors.debug", true).unwrap();
//         let asys = ActorSystem::with_config(&model, "my-system", &cfg);
//         assert!(asys.is_err());
//     }

//     #[test]
//     fn create_system_default() {
//         let model = MockModel;

//         let system = ActorSystem::new(&model).unwrap();
//         assert_eq!(system.name(), "system");
//     }

//     #[test]
//     fn create_system_with_name() {
//         let model = MockModel;

//         let system = ActorSystem::with_name(&model, "my-system").unwrap();
//         assert_eq!(system.name(), "my-system");
//     }

//     #[test]
//     fn create_system_with_config() {
//         let model = MockModel;

//         let system =
//             ActorSystem::with_config(&model, "system", &get_test_config()).unwrap();
//         assert_eq!(system.name(), "system");
//     }

//     #[test]
//     fn select_from_system() {
//         let model = MockModel;

//         let system =
//             ActorSystem::with_config(&model, "my-system", &get_test_config()).unwrap();

//         // select test actor through actor selection: /root/user/child_a
//         system.select("child_a").unwrap();
//     }

//     #[test]
//     fn select_from_system_bad_name() {
//         let model = MockModel;

//         let system =
//             ActorSystem::with_config(&model, "my-system", &get_test_config()).unwrap();

//         // select test actor through actor selection: /root/user/child_a
//         assert!(system.select("@#$@#$@").is_err());
//     }
// }