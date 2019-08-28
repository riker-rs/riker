#![allow(unused_variables)]
use async_trait::async_trait;
use crate::{
    actor::{
        actor_cell::Context,
        actor_ref::{BasicActorRef, Sender},
    },
    system::SystemMsg,
    Message,
};

#[async_trait]
pub trait Actor: Send + 'static {
    type Msg: Message;

    /// Invoked when an actor is being started by the system.
    ///
    /// Any initialization inherent to the actor's role should be
    /// performed here.
    ///
    /// Panics in `pre_start` do not invoke the
    /// supervision strategy and the actor will be terminated.
    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has started.
    ///
    /// Any post initialization can be performed here, such as writing
    /// to a log file, emmitting metrics.
    ///
    /// Panics in `post_start` follow the supervision strategy.
    async fn post_start(&mut self, ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has been stopped.
    async fn post_stop(&mut self) {}

    /// Return a supervisor strategy that will be used when handling failed child actors.
    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }

    /// Invoked when an actor receives a system message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `recv` and `sys_recv`.
    async fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Sender) {}

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `recv` and `sys_recv`.
    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender);
}

#[async_trait]
impl<A: Actor + ?Sized> Actor for Box<A> {
    type Msg = A::Msg;

    async fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).pre_start(ctx).await;
    }

    async fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).post_start(ctx).await
    }

    async fn post_stop(&mut self) {
        (**self).post_stop().await
    }

    async fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
        (**self).sys_recv(ctx, msg, sender).await
    }

    fn supervisor_strategy(&self) -> Strategy {
        (**self).supervisor_strategy()
    }

    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        (**self).recv(ctx, msg, sender).await
    }
}

/// Receive and handle a specific message type
///
/// This trait is typically used in conjuction with the #[actor]
/// attribute macro and implemented for each message type to receive.
///
/// # Examples
///
/// ```
/// use riker::actors::*;
/// use async_trait::async_trait;
///
/// #[derive(Clone, Debug)]
/// struct Foo;
/// #[derive(Clone, Debug)]
/// struct Bar;
/// #[actor(Foo, Bar)] // <-- set our actor to receive Foo and Bar types
/// struct MyActor;
///
/// #[async_trait]
/// impl Actor for MyActor {
///     type Msg = MyActorMsg; // <-- MyActorMsg is provided for us
///
///     async fn recv(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Self::Msg,
///                 sender: Sender) {
///         self.receive(ctx, msg, sender).await; // <-- call the respective implementation
///     }
/// }
///
/// impl MyActor {
///     fn actor() -> Self {
///         MyActor
///     }
///
///     fn props() -> BoxActorProd<MyActor> {
///         Props::new(MyActor::actor)
///     }
/// }
///
/// #[async_trait]
/// impl Receive<Foo> for MyActor {
///     type Msg = MyActorMsg;
///
///     async fn receive(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Foo, // <-- receive Foo
///                 sender: Sender) {
///         println!("Received a Foo");
///     }
/// }
///
/// #[async_trait]
/// impl Receive<Bar> for MyActor {
///     type Msg = MyActorMsg;
///
///     async fn receive(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Bar, // <-- receive Bar
///                 sender: Sender) {
///         println!("Received a Bar");
///     }
/// }
///
/// // main
/// let sys = ActorSystem::new().unwrap();
/// let actor = sys.actor_of(MyActor::props(), "my-actor").unwrap();
///
/// actor.tell(Foo, None);
/// actor.tell(Bar, None);
/// ```
#[async_trait]
pub trait Receive<Msg: Message> {
    type Msg: Message;

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    async fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Msg, sender: Sender);
}

/// The actor trait object
pub type BoxActor<Msg> = Box<dyn Actor<Msg = Msg> + Send>;

/// Supervision strategy
///
/// Returned in `Actor.supervision_strategy`
pub enum Strategy {
    /// Stop the child actor
    Stop,

    /// Attempt to restart the child actor
    Restart,

    /// Escalate the failure to a parent
    Escalate,
}
