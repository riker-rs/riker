#![allow(unused_variables)]

use crate::protocol::{Message, SystemMsg, ActorMsg};
use crate::actor::actor_ref::ActorRef;
use crate::actor::actor_cell::{Context, PersistenceConf};

/// An Actor represents a struct that will be scheduled for execution when it is sent a message.
/// 
/// Actors expose an API through messaging. The only means to interact with
/// an actor is through the messaging protocol via the actor's reference,
/// an `ActorRef`.
/// 
/// When `ActorRef.tell` is used to send a message to an actor, the
/// message is placed in the actor's mailbox and the actor is scheduled
/// for execution.
/// 
/// When the actor is executed the `receive` function is invoked for each
/// message.
pub trait Actor : Send {
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

    /// Invoked when an actor receives a message
    /// 
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<ActorRef<Self::Msg>>);

    /// Invoked when an actor receives a Riker predefined message
    /// 
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn other_receive(&mut self, ctx: &Context<Self::Msg>, msg: ActorMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {

    }

    /// Invoked when an actor receives a Riker system message
    /// 
    /// It is guaranteed that only one message in the actor's mailbox is processed
    /// at any one time, including `receive`, `other_receive` and `system_receive`.
    fn system_receive(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {
        
    }

    /// Return a Some(PersistenceConf) to enable actor persistence.
    ///
    /// # Examples
    /// 
    /// ```
    /// # use riker::actors::*;
    /// struct User {
    ///     id: String,
    /// }
    /// 
    /// impl Actor for User {
    ///     type Msg = String;
    /// 
    ///     fn persistence_conf(&self) -> Option<PersistenceConf> {
    ///         Some(PersistenceConf {
    ///             id: self.id.clone(),
    ///             keyspace: "user".into()
    ///         })
    ///     }
    /// 
    /// #   fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<ActorRef<Self::Msg>>) {}
    /// }
    /// ```
    /// Events persisted using `ctx.persist_event()` will be replayed in order by
    /// passing to `apply_event` when an actor is started, or restarted by a supervisor
    fn persistence_conf(&self) -> Option<PersistenceConf> {
        None
    }

    /// Invoked after an event is successfully inserted into the event store.
    /// 
    /// State changes are safe to make here.
    /// 
    /// Since you should only change state (e.g. self.some_val) when you know
    /// the event has been successfully stored in the event store, `apply_event`
    /// is the only place that this is guaranteed after `ctx.persist_event()`.
    /// 
    /// `ctx.persist_event()` stops further processing of the actor's mailbox messages
    /// until the event is successfully inserted in to the event store. Thus you are
    /// guaranteed that `apply_event` is invoked before the next message is received.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use riker::actors::*;
    /// 
    /// struct Sensor {
    ///     id: String,
    ///     last: u32,
    ///     count: u32,
    ///     avg: f64,
    /// }
    /// 
    /// impl Actor for Sensor {
    ///     type Msg = u32;
    /// 
    ///     fn persistence_conf(&self) -> Option<PersistenceConf> {
    ///         Some(PersistenceConf {
    ///             id: self.id.clone(),
    ///             keyspace: "sensor_1".into(),
    ///         })
    ///     }
    /// 
    ///     fn receive(&mut self,
    ///                 ctx: &Context<Self::Msg>,
    ///                 msg: Self::Msg,
    ///                 sender: Option<ActorRef<Self::Msg>>) {
    ///         // Receive a new sensor reading and store it
    ///         ctx.persist_event(msg);
    ///     }
    /// 
    ///     fn apply_event(&mut self, _ctx: &Context<Self::Msg>, evt: Self::Msg) {
    ///         // Sensor reading has been stored
    ///         // Local state can be updated
    ///         self.last = evt;
    ///         self.count += 1;
    ///         self.avg = (self.last / self.count).into();
    ///     }
    /// }
    /// ```
    fn apply_event(
        &mut self,
        ctx: &Context<Self::Msg>,
        evt: Self::Msg,
        sender: Option<ActorRef<Self::Msg>>
    ) {
    
    }

    /// Invoked for each event when the actor is recovering.
    /// 
    /// State changes are safe to make here.
    /// 
    /// Since you should only change state (e.g. `self.some_val`) when you know
    /// the event exists in the event store, `replay_event`
    /// is safe to change the state.
    /// 
    /// `replay_event` is used instead of `apply_event` when recovering
    /// to allow for different bahavior. Typically replaying should only
    /// be used to change state for the purpose of recovering the actor's state
    /// and not perform additional messaging.
    ///
    /// # Examples
    /// 
    /// ```
    /// # use riker::actors::*;
    /// struct Sensor {
    ///     id: String,
    ///     last: u32,
    ///     count: u32,
    ///     avg: f64,
    /// }
    /// 
    /// impl Actor for Sensor {
    ///     type Msg = u32;
    /// 
    ///     fn persistence_conf(&self) -> Option<PersistenceConf> {
    ///         Some(PersistenceConf {
    ///             id: self.id.clone(),
    ///             keyspace: "sensor_1".into(),
    ///         })
    ///     }
    /// 
    ///     fn receive(&mut self,
    ///                 ctx: &Context<Self::Msg>,
    ///                 msg: Self::Msg,
    ///                 sender: Option<ActorRef<Self::Msg>>) {
    ///         // Receive a new sensor reading and store it
    ///         ctx.persist_event(msg);
    ///     }
    ///
    ///     fn replay_event(&mut self, _ctx: &Context<Self::Msg>, evt: Self::Msg) {
    ///         // Received a previously stored sensor reading
    ///         // Update local state
    ///         self.last = evt;
    ///         self.count += 1;
    ///         self.avg = (self.last / self.count).into();
    ///     }
    /// }
    /// ```
    fn replay_event(&mut self, ctx: &Context<Self::Msg>, evt: Self::Msg) {
    
    }

    /// Return a supervisor strategy that will be used when handling failed child actors.
    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Restart
    }
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

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<ActorRef<Self::Msg>>) {
        (**self).receive(ctx, msg, sender);
    }

    fn other_receive(&mut self, ctx: &Context<Self::Msg>, msg: ActorMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {
        (**self).other_receive(ctx, msg, sender);
    }

    fn system_receive(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, sender: Option<ActorRef<Self::Msg>>) {
        (**self).system_receive(ctx, msg, sender)
    }

    fn persistence_conf(&self) -> Option<PersistenceConf> {
        (**self).persistence_conf()
    }

    fn apply_event(
        &mut self,
        ctx: &Context<Self::Msg>,
        evt: Self::Msg,
        sender: Option<ActorRef<Self::Msg>>
    ) {
        (**self).apply_event(ctx, evt, sender)
    }

    fn replay_event(&mut self, ctx: &Context<Self::Msg>, evt: Self::Msg) {
        (**self).replay_event(ctx, evt)
    }

    fn supervisor_strategy(&self) -> Strategy {
        (**self).supervisor_strategy()
    }
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
