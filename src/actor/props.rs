use std::sync::{Arc, Mutex};
use std::fmt;
use std::panic::{UnwindSafe, RefUnwindSafe};

use crate::actor::{Actor, BoxActor};

/// Provides instances of `ActorProducer` for use when creating Actors (`actor_of`).
/// 
/// Actors are not created directly. Instead you provide an `ActorProducer`
/// that allows the `ActorSystem` to start an actor when `actor_of` is used,
/// or when an actor fails and a supervisor requests an actor to be restarted.
/// 
/// `ActorProducer` can hold values required by the actor's factory method
/// parameters.
pub struct Props;

impl Props {
    /// Creates an `ActorProducer` with no factory method parameters.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # extern crate riker;
    /// # extern crate riker_default;
    /// 
    /// # use riker::actors::*;
    /// # use riker_default::DefaultModel;
    /// 
    /// struct User;
    /// 
    /// impl User {
    ///     fn actor() -> BoxActor<String> {
    ///         Box::new(User)
    ///     }
    /// }
    /// 
    /// # impl Actor for User {
    /// #    type Msg = String;
    /// #    fn receive(&mut self, _ctx: &Context<String>, _msg: String, _sender: Option<ActorRef<String>>) {}
    /// # }
    /// // main
    /// let model: DefaultModel<String> = DefaultModel::new();
    /// let system = ActorSystem::new(&model).unwrap();
    ///
    /// let props = Props::new(Box::new(User::actor));
    /// 
    /// // start the actor and get an `ActorRef`
    /// let actor = system.actor_of(props, "user").unwrap();
    /// ```
    pub fn new<A>(creator: Box<Fn() -> A + Send>)
        -> Arc<Mutex<Box<ActorProducer<Actor = A>>>>
        where A: Actor + Send + 'static
    {
        Arc::new(Mutex::new(ActorProps::new(creator)))
    }

    /// Creates an `ActorProducer` with one or more factory method parameters.
    /// 
    /// # Examples
    /// An actor requiring a single parameter.
    /// ```
    /// # extern crate riker;
    /// # extern crate riker_default;
    /// 
    /// # use riker::actors::*;
    /// # use riker_default::DefaultModel;
    /// 
    /// struct User {
    ///     name: String,
    /// }
    /// 
    /// impl User {
    ///     fn actor(name: String) -> BoxActor<String> {
    ///         let actor = User {
    ///             name
    ///         };
    /// 
    ///         Box::new(actor)
    ///     }
    /// }
    /// 
    /// # impl Actor for User {
    /// #    type Msg = String;
    /// #    fn receive(&mut self, _ctx: &Context<String>, _msg: String, _sender: Option<ActorRef<String>>) {}
    /// # }
    /// // main
    /// let model: DefaultModel<String> = DefaultModel::new();
    /// let system = ActorSystem::new(&model).unwrap();
    ///
    /// let props = Props::new_args(Box::new(User::actor), "Naomi Nagata".into());
    /// 
    /// let actor = system.actor_of(props, "user").unwrap();
    /// ```
    /// An actor requiring multiple parameters.
    /// ```
    /// # extern crate riker;
    /// # extern crate riker_default;
    /// 
    /// # use riker::actors::*;
    /// # use riker_default::DefaultModel;
    /// 
    /// struct BankAccount {
    ///     name: String,
    ///     number: String,
    /// }
    /// 
    /// impl BankAccount {
    ///     fn actor((name, number): (String, String)) -> BoxActor<String> {
    ///         let actor = BankAccount {
    ///             name,
    ///             number
    ///         };
    /// 
    ///         Box::new(actor)
    ///     }
    /// }
    /// 
    /// # impl Actor for BankAccount {
    /// #    type Msg = String;
    /// #    fn receive(&mut self, _ctx: &Context<String>, _msg: String, _sender: Option<ActorRef<String>>) {}
    /// # }
    /// // main
    /// let model: DefaultModel<String> = DefaultModel::new();
    /// let system = ActorSystem::new(&model).unwrap();
    ///
    /// let props = Props::new_args(Box::new(BankAccount::actor),
    ///                             ("James Holden".into(), "12345678".into()));
    /// 
    /// // start the actor and get an `ActorRef`
    /// let actor = system.actor_of(props, "bank_account").unwrap();
    /// ```
    pub fn new_args<A, Args>(creator: Box<Fn(Args) -> A + Send>, args: Args)
        -> Arc<Mutex<Box<ActorProducer<Actor = A>>>>
        where A: Actor + Send + 'static, Args: ActorArgs + 'static        
    {
        Arc::new(Mutex::new(ActorPropsWithArgs::new(creator, args)))
    }
}

/// A `Clone`, `Send` and `Sync` `ActorProducer`
pub type BoxActorProd<Msg> = Arc<Mutex<ActorProducer<Actor=BoxActor<Msg>>>>;

/// Represents the underlying Actor factory function for creating instances of `Actor`.
/// 
/// Actors are not created directly. Instead you provide an `ActorProducer`
/// that allows the `ActorSystem` to start an actor when `actor_of` is used,
/// or when an actor fails and a supervisor requests an actor to be restarted.
/// 
/// `ActorProducer` can hold values required by the actor's factory method
/// parameters.
pub trait ActorProducer : fmt::Debug + Send + UnwindSafe + RefUnwindSafe {
    type Actor: Actor;

    /// Produces an instance of an `Actor`.
    /// 
    /// The underlying factory method provided
    /// in the original `Props::new(f: Fn() -> A + Send`) or
    /// `Props::new(f: Fn(Args) -> A + Send>, args: Args)` is called.
    /// 
    /// Any parameters `Args` will be cloned and passed to the function.
    /// 
    /// # Panics
    /// If the provided factory method panics the panic will be caught
    /// by the system, resulting in an error result returning to `actor_of`.
    fn produce(&self) -> Self::Actor;
}

impl<A> ActorProducer for Arc<Mutex<Box<ActorProducer<Actor = A>>>>
    where A: Actor + Send + 'static
{
    type Actor = A;

    fn produce(&self) -> A {
        self.lock().unwrap().produce()
    }
}

impl<A> ActorProducer for Arc<Mutex<ActorProducer<Actor = A>>>
    where A: Actor + Send + 'static
{
    type Actor = A;

    fn produce(&self) -> A {
        self.lock().unwrap().produce()
    }
}

impl<A> ActorProducer for Box<ActorProducer<Actor = A>>
    where A: Actor + Send + 'static
{
    type Actor = A;

    fn produce(&self) -> A {
        (**self).produce()
    }
}

pub struct ActorProps<A: Actor> {
    creator: Box<Fn() -> A + Send>,
}

impl<A: Actor> UnwindSafe for ActorProps<A> {}
impl<A: Actor> RefUnwindSafe for ActorProps<A> {}

impl<A> ActorProps<A> 
    where A: Actor + Send + 'static
{
    pub fn new(creator: Box<Fn() -> A + Send>) -> Box<ActorProducer<Actor = A>> {
        Box::new(ActorProps { creator: creator })
    }
}

impl<A> ActorProducer for ActorProps<A>
    where A: Actor + Send + 'static
{
    type Actor = A;

    fn produce(&self) -> A {
        let ref f = self.creator;
        f()
    }
}

impl<A: Actor> fmt::Display for ActorProps<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Props") //todo fix this
    }
}

impl<A: Actor> fmt::Debug for ActorProps<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Props") //todo fix this
    }
}

pub struct ActorPropsWithArgs<A: Actor, Args: ActorArgs> {
    creator: Box<Fn(Args) -> A + Send>,
    args: Args,
}

impl<A: Actor, Args: ActorArgs> UnwindSafe for ActorPropsWithArgs<A, Args> {}
impl<A: Actor, Args: ActorArgs> RefUnwindSafe for ActorPropsWithArgs<A, Args> {}

impl<A, Args> ActorPropsWithArgs<A, Args>
    where A: Actor + Send + 'static, Args: ActorArgs + 'static
{
    pub fn new(creator: Box<Fn(Args) -> A + Send>, args: Args) -> Box<ActorProducer<Actor = A>> {
        Box::new(ActorPropsWithArgs {
            creator: creator,
            args: args,
        })
    }
}

impl<A, Args> ActorProducer for ActorPropsWithArgs<A, Args>
    where A: Actor + Send + 'static, Args: ActorArgs + 'static
{
    type Actor = A;

    fn produce(&self) -> A {
        let ref f = self.creator;
        let args = self.args.clone();
        f(args)
    }
}

impl<A: Actor, Args: ActorArgs> fmt::Display for ActorPropsWithArgs<A, Args> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Props") //todo fix this
    }
}

impl<A: Actor, Args: ActorArgs> fmt::Debug for ActorPropsWithArgs<A, Args> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Props") //todo fix this
    }
}

pub trait ActorArgs: Clone + Send + Sync {}
impl<T: Clone + Send + Sync> ActorArgs for T {}

