use std::fmt::Debug;
use std::fmt;
use std::marker::PhantomData;

use async_trait::async_trait;
use riker_macros::actor;

#[actor(String, u32)]
#[derive(Clone)]
struct NewActor;

impl NewActor {
    fn actor() -> Self {
        NewActor
    }  
}

#[async_trait]
impl Actor for NewActor {
    type Msg = NewActorMsg;

    async fn recv(&mut self,
                ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                sender: Sender) {

        println!("handling..");
        self.receive(ctx, msg, sender).await;
    }
}

#[async_trait]
impl Receive<u32> for NewActor {
    type Msg = NewActorMsg;

    async fn receive(&mut self,
                _ctx: &Context<Self::Msg>,
                _msg: u32,
                _sender: Sender) {
        println!("u32");
    }
}

#[async_trait]
impl Receive<String> for NewActor {
    type Msg = NewActorMsg;

    async fn receive(&mut self,
                _ctx: &Context<Self::Msg>,
                _msg: String,
                _sender: Sender) {
        println!("String");
    }
}

// ------------------------------------------------------------------------------------------------

struct BasicActorRef;
type Sender = Option<BasicActorRef>;

struct Context<Msg: Message> { x: PhantomData<Msg> }
unsafe impl<Msg: Message> Sync for Context<Msg> {}

#[async_trait]
trait Actor: Send + 'static {
    type Msg: Message;

    /// Invoked when an actor is being started by the system.
    ///
    /// Any initialization inherent to the actor's role should be
    /// performed here.
    ///
    /// Panics in `pre_start` do not invoke the
    /// supervision strategy and the actor will be terminated.
    async fn pre_start(&mut self, _ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has started.
    ///
    /// Any post initialization can be performed here, such as writing
    /// to a log file, emitting metrics.
    ///
    /// Panics in `post_start` follow the supervision strategy.
    async fn post_start(&mut self, _ctx: &Context<Self::Msg>) {}

    /// Invoked after an actor has been stopped.
    fn post_stop(&mut self) {}

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `recv` and `sys_recv`.
    async fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender);
}

#[async_trait]
trait Receive<Msg: Message> {
    type Msg: Message;

    /// Invoked when an actor receives a message
    ///
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    async fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Msg, sender: Sender);
}

type BoxedTell<T> = Box<dyn Tell<T> + Send + 'static>;

#[async_trait]
trait Tell<T>:  Send + 'static {
    async fn tell(&self, msg: T, sender: Option<BasicActorRef>);
    fn box_clone(&self) -> BoxedTell<T>;
}

impl<T> fmt::Debug for BoxedTell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tell")
    }
}

impl<T> fmt::Display for BoxedTell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tell")
    }
}

impl<T: 'static> Clone for BoxedTell<T> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

trait Message: Debug + Clone + Send + 'static {}
impl<T: Debug + Clone + Send + 'static> Message for T {}

#[derive(Clone)]
struct ActorRef<T: Message> {
    x: PhantomData<T>,
}

impl<T: Message> ActorRef<T> {
    async fn send_msg(&self, msg: T, sender: Option<BasicActorRef>) {
        let a = NewActor::actor();
    }
} 