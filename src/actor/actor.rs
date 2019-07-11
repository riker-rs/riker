#![allow(unused_variables)]

use crate::{
    Message,
    system::SystemMsg,
    actor::{
        actor_ref::{BasicActorRef, Sender},
        actor_cell::Context
    }
};

pub trait Actor: Send + 'static {
    type Msg: Message;

    /// Invoked when an actor is being started by the system.
    ///
    /// Any initialization inherent to the actor's role should be
    /// performed here.
    /// 
    /// Panics in `pre_start` do not invoke the
    /// supervision strategy and the actor will be terminated.
    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {

    }

    /// Invoked after an actor has started.
    ///
    /// Any post initialization can be performed here, such as writing
    /// to a log file, emmitting metrics.
    /// 
    /// Panics in `post_start` follow the supervision strategy.
    fn post_start(&mut self, ctx: &Context<Self::Msg>) {

    }

    /// Invoked after an actor has been stopped.
    fn post_stop(&mut self) {

    }

    fn sys_recv(&mut self,
                    ctx: &Context<Self::Msg>,
                    msg: SystemMsg,
                    sender: Sender) {
        
    }

    /// Return a supervisor strategy that will be used when handling failed child actors.
    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }

    fn recv(&mut self,
                ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                sender: Sender);
}

impl<A: Actor + ?Sized> Actor for Box<A> {
    type Msg = A::Msg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).pre_start(ctx);
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        (**self).post_start(ctx)
    }

    fn post_stop(&mut self) {
        (**self).post_stop()
    }

    fn sys_recv(&mut self,
                    ctx: &Context<Self::Msg>,
                    msg: SystemMsg,
                    sender: Option<BasicActorRef>) {
        (**self).sys_recv(ctx, msg, sender)
    }

    fn supervisor_strategy(&self) -> Strategy {
        (**self).supervisor_strategy()
    }

    fn recv(&mut self,
                ctx: &Context<Self::Msg>,
                msg: Self::Msg,
                sender: Option<BasicActorRef>) {
        (**self).recv(ctx, msg, sender)
    }
}

pub trait Receive<Msg: Message> {
    type Msg: Message;

    /// Invoked when an actor receives a message
    /// 
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn receive(&mut self,
                ctx: &Context<Self::Msg>,
                msg: Msg,
                sender: Option<BasicActorRef>);
}

/// The actor trait object
pub type BoxActor<Msg> = Box<dyn Actor<Msg=Msg> + Send>;

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
