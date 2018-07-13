use protocol::Message;
use kernel::Dispatcher;
use system::{LoggerProps, DeadLetterProps};
use system::EventStore;
use system::IoManagerProps;
use system::TimerFactory;

/// Riker's system and module configuration.
/// 
/// Riker requires a `Model` to set the message type used
/// throughout the system and specify modules that provide
/// core services.
/// 
/// A default model is provided by the `riker-default` crate that
/// allows you to specify your message type (protocol).
/// 
/// If you prefer to use your own module for any of the core services
/// you can do so easily by creating your own model by implementing `Model`.
/// 
/// Examples
/// 
/// Using the default model:
/// ```
/// extern crate riker;
/// extern crate riker_default;
/// 
/// use riker::actors::*;
/// use riker_default::DefaultModel;
/// 
/// // Get a default model with String as the message type
/// let model: DefaultModel<String> = DefaultModel::new();
/// let sys = ActorSystem::new(&model).unwrap();
/// ```
/// 
/// Implementing your own model:
/// ```ignore
/// extern crate riker;
/// extern crate riker_default;
/// 
/// use riker::actors::*;
/// use riker_default::*; // <-- we're going to use some default modules
/// 
/// struct MyModel;
/// 
/// impl Model for MyModel {
///     type Msg = String;
///     type Dis = ThreadPoolDispatcher;
///     type Ded = DeadLettersActor<Self::Msg>;
///     type Tmr = BasicTimer<Self::Msg>;
///     type Evs = Redis<Self::Msg>; // <-- we're using a module to provide Redis storage 
///     type Tcp = TcpManager<Self::Msg>;
///     type Udp = TcpManager<Self::Msg>;
///     type Log = MyLogger<Self::Msg>; // <-- we're using our own Log module
/// }
/// 
/// let sys = ActorSystem::new(&MyModel).unwrap();
/// ```
pub trait Model : Sized {
    /// The message type used throughout the system.
    /// `Actor.receive` expects this type
    type Msg: Message;

    /// Dispatcher executes actors and futures
    type Dis: Dispatcher;

    /// Logger provides global logging, e.g. info!("hello");
    type Log: LoggerProps<Msg = Self::Msg>;

    /// Dead letters subscribes to the dead letters channel
    type Ded: DeadLetterProps<Msg = Self::Msg>;

    /// Timer provides message scheduling, e.g. `ctx.schedule_once`
    type Tmr: TimerFactory<Msg = Self::Msg>;

    /// Event store provides the storage system for events/messages
    type Evs: EventStore<Msg=Self::Msg>;

    type Tcp: IoManagerProps<Msg = Self::Msg>;
    type Udp: IoManagerProps<Msg = Self::Msg>;
}

