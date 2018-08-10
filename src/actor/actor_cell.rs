use std::fmt;
use std::time::{SystemTime, Duration};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::HashMap;
use std::ops::Deref;

use chrono::{DateTime, Utc};
use uuid::Uuid;
use rand;
use futures::{Future, FutureExt};

use protocol::*;
use ExecutionContext;
use actor::*;
use actor::dead_letter;
use kernel::{KernelRef, MailboxSender};
use system::{ActorSystem, Evt, query};
use system::{Timer, Job, OnceJob, RepeatJob};
use futures_util::DispatchHandle;
use validate::{validate_name, InvalidPath};

#[derive(Clone)]
pub struct ActorCell<Msg: Message> {
    inner: Arc<ActorCellInner<Msg>>,
}

#[derive(Clone)]
struct ActorCellInner<Msg: Message> {
    uid: ActorId,
    uri: ActorUri,
    parent: Option<ActorRef<Msg>>,
    children: Children<Msg>,
    is_remote: bool,
    is_terminating: Arc<AtomicBool>,
    is_restarting: Arc<AtomicBool>,
    persistence: Persistence<Msg>,
    status: Arc<AtomicUsize>,
    kernel: KernelRef<Msg>,
    system: ActorSystem<Msg>,
    mailbox: Option<MailboxSender<Msg>>,
}

#[derive(Clone)]
pub struct Persistence<Msg: Message> {
    pub event_store: Option<ActorRef<Msg>>,
    pub is_persisting: Arc<AtomicBool>,
    pub persistence_conf: Option<PersistenceConf>,
}

impl<Msg> ActorCell<Msg>
    where Msg: Message
{
    /// Constructs a new `ActorCell`
    pub fn new(uid: ActorId,
            uri: ActorUri,
            parent: Option<ActorRef<Msg>>,
            kernel: &KernelRef<Msg>,
            system: &ActorSystem<Msg>,
            perconf: Option<PersistenceConf>,
            mailbox: Option<MailboxSender<Msg>>)
            -> ActorCell<Msg> {

        let inner = ActorCellInner {
            uid,
            uri,
            parent,
            children: Children::new(),
            is_remote: false,
            is_terminating: Arc::new(AtomicBool::new(false)),
            is_restarting: Arc::new(AtomicBool::new(false)),
            persistence: Persistence {
                event_store: system.event_store.clone(),
                is_persisting: Arc::new(AtomicBool::new(false)),
                persistence_conf: perconf,
            },
            status: Arc::new(AtomicUsize::new(0)),
            kernel: kernel.clone(),
            system: system.clone(),
            mailbox: mailbox
        };

        ActorCell {
            inner: Arc::new(inner)
        }
    }
}

/// `ActorCell` internal API.
/// 
/// This trait is used by the internal system to provode
/// control over `Actor` and `ActorCell`
pub trait CellInternal {
    type Msg: Message;

    /// Return the system's dead letters reference.
    fn dead_letters(&self) -> &ActorRef<Self::Msg>;

    /// Return the actor's persistence configuration.
    fn persistence_conf(&self) -> Option<PersistenceConf>;

    /// Returns true if the actor is currently in a state of persisting.
    fn is_persisting(&self) -> bool;

    /// Sets the persisting status of the actor.
    /// 
    /// This signals to the actor's mailbox
    /// to defer processing of messages until the event store
    /// completes storing the event.
    fn set_persisting(&self, b: bool);

    /// Invoked when an actor receives an `Identify` message.
    /// 
    /// message. An `Info` message must be sent to the `sender`,
    /// with the sender of the message set to `myself`.
    fn identify(&self, sender: Option<ActorRef<Self::Msg>>);

    /// Adds a child under this actor.
    fn add_child(&self, name: &str, actor: ActorRef<Self::Msg>);

    /// Send an `ActorCmd::Stop` to the given actor.
    fn stop(&self, actor: ActorRef<Self::Msg>);


    /// Invoked when the actor receives a command.
    /// 
    /// Possible commands:
    /// - `Stop`: attempt to terminate the actor
    /// - `Restart`: attempt to restart the actor
    fn receive_cmd(&self, cmd: ActorCmd, actor: &mut Option<BoxActor<Self::Msg>>);

    /// Terminate the actor associated with this `ActorCell`.
    /// 
    /// If the actor has no children then the kernel is
    /// instructed to terminate the actor immediately.
    /// 
    /// If the actor has children then each child is
    /// sent a stop command and the actor is placed in
    /// a 'terminating' state. When the actor is notified of
    /// each child's termination it checks to see if there
    /// are no more children so it can safely stop itself.
    /// 
    /// Does not block.
    fn terminate(&self, actor: &mut Option<BoxActor<Self::Msg>>);

    /// Restart the actor associated with this `ActorCell`
    /// 
    /// If the actor has no children then the kernel is
    /// instructed to restart the actor immediately.
    /// 
    /// If the actor has children then each child is
    /// sent a stop command and the actor is placed in
    /// a 'restarting' state. When the actor is notified of
    /// each child's termination it checks to see if there
    /// are no more children so it can safely restart itself.
    /// 
    /// Does not block.
    fn restart(&self);

    /// Invoked when a child actor is terminated.
    /// 
    /// Each time an actor is stopped, either manually or as
    /// part of supervision, its parent is notified.
    /// 
    /// If the actor is in a state of terminating or restarting
    /// it will check to see if those operations can be completed
    /// after all children have been terminated.
    fn death_watch(&self, terminated: &ActorRef<Self::Msg>, actor: &mut Option<BoxActor<Self::Msg>>);
    
    /// Invoked when a child actor fails (panics).
    /// 
    /// The provided supervision strategy will be executed.
    fn handle_failure(&self, failed: ActorRef<Self::Msg>, strategy: Strategy);

    /// Invoked when the supervision strategy restarts a child actor.
    fn restart_child(&self, actor: ActorRef<Self::Msg>);

    /// Invoked when the supervision strategy escalates an actor's failure.
    fn escalate_failure(&self);

    fn is_child(&self, actor: &ActorRef<Self::Msg>) -> bool;

    /// Invoked to query an actor's events during actor start.
    /// 
    /// If an actor has persistence configured
    /// its events are queried from the data store and sent to
    /// the actor to complete actor initialization.
    /// 
    /// Must not block.
    fn load_events(&self, actor: &mut Option<BoxActor<Self::Msg>>) -> bool;

    /// Invoked during actor start to complete actor initialization.
    /// 
    /// Applies only in the case where persistence is configured.
    /// 
    /// `replay` is called when the event store query, created in
    /// `load_events` has completed and the values are available.
    fn replay(&self, ctx: &Context<Self::Msg>, evts: Vec<Self::Msg>, actor: &mut Option<BoxActor<Self::Msg>>); 
}

impl<Msg> CellInternal for ActorCell<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn dead_letters(&self) -> &ActorRef<Msg> {
        self.inner.system.dead_letters()
    }

    fn persistence_conf(&self) -> Option<PersistenceConf> {
        self.inner.persistence.persistence_conf.clone()
    }

    fn is_persisting(&self) -> bool {
        self.inner.persistence.is_persisting.load(Ordering::Relaxed)
    }

    fn set_persisting(&self, b: bool) {
        self.inner.persistence.is_persisting.store(b, Ordering::Relaxed);
    }

    fn identify(&self, sender: Option<ActorRef<Msg>>) {
        if let Some(s) = sender {
            s.tell(Info, Some(self.myself()));
        }
    }

    fn add_child(&self, name: &str, actor: ActorRef<Msg>) {
        self.inner.children.add(name, actor);
    }

    fn stop(&self, actor: ActorRef<Msg>) {
        let sys_msg = SystemMsg::ActorCmd(ActorCmd::Stop);
        actor.sys_tell(sys_msg, None);
    }

    fn receive_cmd(&self, cmd: ActorCmd, actor: &mut Option<BoxActor<Msg>>) {
        match cmd {
            ActorCmd::Stop => self.terminate(actor),
            ActorCmd::Restart => self.restart()
        }
    }

    fn terminate(&self, actor: &mut Option<BoxActor<Msg>>) {
        // *1. Suspend non-system mailbox messages
        // *2. Iterate all children and send ActorCmd::Stop to each
        // *3. Wait for Event::ActorTerminated from each child

        self.inner.is_terminating.store(true, Ordering::Relaxed);

        if self.inner.children.count() == 0 {
            self.inner.kernel.terminate_actor(self.inner.uid);
            post_stop(actor);
        } else {
            for child in Box::new(self.inner.children.iter().clone()) {
                self.stop(child.clone());
            }
        }
    }

    fn restart(&self) {
        if self.inner.children.count() == 0 {
            self.inner.kernel.restart_actor(self.inner.uid);
        } else {
            self.inner.is_restarting.store(true, Ordering::Relaxed);
            for child in Box::new(self.inner.children.iter().clone()) {
                self.stop(child.clone());
            }
        }
    }

    fn death_watch(&self, terminated: &ActorRef<Msg>, actor: &mut Option<BoxActor<Msg>>) {
        if self.is_child(&terminated) {
            let children = &self.inner.children;
            children.remove(&terminated.uri.name);

            if children.count() == 0 {
                // if the actor is terminating the kernel can be notified to complete termination
                if self.inner.is_terminating.load(Ordering::Relaxed) {
                    self.inner.kernel.terminate_actor(self.inner.uid);
                    post_stop(actor);
                }

                // if the actor is restarting the kernel can be notified to restart the actor
                if self.inner.is_restarting.load(Ordering::Relaxed) {
                    self.inner.is_restarting.store(false, Ordering::Relaxed);
                    self.inner.kernel.restart_actor(self.inner.uid);
                }
            }
        }
    }

    fn handle_failure(&self, failed: ActorRef<Msg>, strategy: Strategy) {
        match strategy {
            Strategy::Stop => self.stop(failed),
            Strategy::Restart => self.restart_child(failed),
            Strategy::Escalate => self.escalate_failure()
        }
    }

    fn restart_child(&self, actor: ActorRef<Msg>) {
        let sys_msg = SystemMsg::ActorCmd(ActorCmd::Restart);
        actor.sys_tell(sys_msg, None);
    }

    fn escalate_failure(&self) {
        self.inner
            .parent
            .as_ref()
            .unwrap()
            .sys_tell(SystemMsg::Failed(self.myself()), None);
    }

    fn is_child(&self, actor: &ActorRef<Msg>) -> bool {
        self.inner.children.iter().any(|child| child == *actor)
    }

    fn load_events(&self, actor: &mut Option<BoxActor<Msg>>) -> bool {
        let event_store = &self.inner.persistence.event_store;
        let perconf = &self.inner.persistence.persistence_conf;

        match (actor, event_store, perconf) {
            (Some(_), Some(es), Some(perconf)) => {
                let q = query(&perconf.id, &perconf.keyspace, &es, self);
                let myself = self.myself();
                self.execute(
                    q.map(move |evts| {
                        myself.sys_tell(SystemMsg::Replay(evts), None);
                    }));
                
                false
            }
            (Some(_), None, Some(_)) => {
                warn!("Can't load actor events. No event store configured");
                true
            }
            _ => {
                // anything else either the actor is None or there's no persistence configured
                true
            }
        }
    }

    fn replay(&self, ctx: &Context<Msg>, evts: Vec<Msg>, actor: &mut Option<BoxActor<Msg>>) {
        if let Some(actor) = actor.as_mut() {
            for event in evts.iter() {
                actor.replay_event(ctx, event.clone());
            }
        }
    }
}

impl<Msg> ExecutionContext for ActorCell<Msg>
    where Msg: Message
{
    fn execute<F: Future>(&self, f: F) -> DispatchHandle<F::Item, F::Error>
        where F: Future + Send + 'static,
                F::Item: Send + 'static,
                F::Error: Send + 'static,
    {
        self.inner.kernel.execute(f)
    }
}

impl<Msg> TmpActorRefFactory for ActorCell<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn tmp_actor_of(&self, props: BoxActorProd<Msg>) -> Result<ActorRef<Msg>, CreateError> {
        let name = rand::random::<u64>();
        let name = format!("{}", name);

        self.inner.kernel.create_actor(props, &name, &self.inner.system.temp_root())
    }
}

fn post_stop<Msg: Message>(actor: &mut Option<BoxActor<Msg>>) {
    // If the actor instance exists we can execute post_stop.
    // The instance will be None if this is an actor that has failed
    // and is being terminated by an escalated supervisor.
    if let Some(act) = actor.as_mut() {
        act.post_stop();
    }
}

impl<Msg> fmt::Debug for ActorCell<Msg>
    where Msg: Message
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cell = {:?}", "")// todo CELL REFACT
    }
}

/// `ActorCell` public API.
/// 
/// This trait is used by `ActorRef`.
pub trait CellPublic {
    type Msg: Message;

    /// Returns the actor's `ActorRef`
    fn myself(&self) -> ActorRef<Self::Msg>;

    /// Returns the actor's parent `ActorRef`
    fn parent(&self) -> ActorRef<Self::Msg>;

    /// Returns an iterator for the actor's children references
    fn children<'a>(&'a self) -> Box<Iterator<Item = ActorRef<Self::Msg>> + 'a>;

    fn user_root(&self) -> ActorRef<Self::Msg>;

    fn is_root(&self) -> bool;

    /// Adds the given message to the cell actor's mailbox
    #[doc(hidden)]
    fn send_msg(&self, msg: Envelope<Self::Msg>) -> MsgResult<Envelope<Self::Msg>>;

    /// Adds the given system message to the cell actor's mailbox
    #[doc(hidden)]
    fn send_sys_msg(&self, msg: SystemEnvelope<Self::Msg>) -> MsgResult<SystemEnvelope<Self::Msg>>;
}

impl<Msg> CellPublic for ActorCell<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn myself(&self) -> ActorRef<Msg> {
        ActorRef {
            uri: self.inner.uri.clone(),
            cell: self.clone()
        }
    }

    fn parent(&self) -> ActorRef<Msg> {
        self.inner.parent.as_ref().unwrap().clone()
    }

    fn children<'a>(&'a self) -> Box<Iterator<Item = ActorRef<Msg>> + 'a> {
        Box::new(self.inner.children.iter().clone())
    }

    fn user_root(&self) -> ActorRef<Msg> {
        self.inner.system.user_root().clone()
    }

    fn is_root(&self) -> bool {
        self.inner.uid == 0
    }

    fn send_msg(&self, msg: Envelope<Msg>) -> MsgResult<Envelope<Msg>> {
        let result = match self.inner.mailbox {
            Some(ref mbox) => {
                self.inner.kernel.dispatch(msg, mbox)
            }
            None => {
                Err(MsgError::new(msg))
            }
        };

        result.map_err(|e| {
            // TODO candidate for improved code readability
            let sp = e.msg.sender.clone().map(|s| s.uri.path.deref().clone());
            let mp = self.inner.uri.path.deref().clone();
            let dl = self.inner.system.dead_letters();
            dead_letter(dl,
                        sp,
                        mp,
                        e.msg.msg.clone());
            e
        })
    }

    fn send_sys_msg(&self, msg: SystemEnvelope<Msg>) -> MsgResult<SystemEnvelope<Msg>> {
        match self.inner.mailbox {
            Some(ref mbox) => {
                self.inner.kernel.dispatch_sys(msg, mbox)
            }
            None =>
                Err(MsgError::new(msg))
        }
    }
}

/// Provides context, including the actor system during actor execution.
/// 
/// `Context` is passed to an actor's functions, such as
/// `receive`.
/// 
/// Operations performed are in most cases done so from the
/// actor's perspective. For example, creating a child actor
/// using `ctx.actor_of` will create the child under the current
/// actor within the heirarchy. In a similar manner, persistence
/// operations such as `persist_event` use the current actor's
/// persistence configuration.
/// 
/// Since `Context` is specific to an actor and its functions
/// it is not cloneable.  
pub struct Context<Msg: Message> {
    pub myself: ActorRef<Msg>,
    pub system: ActorSystem<Msg>,
    pub persistence: Persistence<Msg>,
    kernel: KernelRef<Msg>,
}

impl<Msg> Context<Msg>
    where Msg: Message
{
    pub fn new(myself: ActorRef<Msg>,
                system: ActorSystem<Msg>,
                persistence: Persistence<Msg>,
                kernel: KernelRef<Msg>) -> Context<Msg> {
        
        Context {
            myself,
            system,
            persistence,
            kernel
        }
    }

    /// Returns the `ActorRef` of the current actor.
    pub fn myself(&self) -> ActorRef<Msg> {
        self.myself.clone()
    }

    /// Persists an event to the event store.
    /// 
    /// In an event sourcing environment state is maintained by
    /// storing the events that change the state. `persist_event`
    /// attempts to store the event.
    /// 
    /// State should not be modified until the event has been
    /// successfully persisted. The event store manager notifies
    /// the actor after each event is stored and the actor's state
    /// can then safely be updated. The actor's `apply_event` method
    /// is called after an event is stored.
    /// 
    /// While an event is persisting it is guaranteed that no other
    /// messages are processed until persistence is complete. I.e.,
    /// no messages are processed between `persist_event` and the
    /// resulting `apply_event`.
    /// 
    /// When an actor starts (or restarts), if pesistence is
    /// configured the system will query the event source manager
    /// to get all the events for the actor. Each event is then replayed,
    /// in their original order, which effectively results in an actor
    /// being in the latest state.
    pub fn persist_event(&self, evt: Msg) {
        let event_store = &self.persistence.event_store;
        let perconf = &self.persistence.persistence_conf;

        match (event_store, perconf) {
            (&Some(ref es), &Some(ref perconf)) => {
                self.persistence.is_persisting.store(true, Ordering::Relaxed);

                let evt = Evt::new(evt);
                es.tell(ESMsg::Persist(evt,
                                        perconf.id.clone(),
                                        perconf.keyspace.clone()
                                        ), Some(self.myself.clone()));
            }
            (&Some(_), &None) => {
                warn!("Can't persist event. No persistence configuration");
            }
            (&None, &Some(_)) => {
                warn!("Can't persist event. No event store manager");
            }
            _ => {
                warn!("Can't persist event. No persistence configuration and no event store manager")
            }

        }
    }
}

impl<Msg> ActorRefFactory for Context<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn actor_of(&self,
                props: BoxActorProd<Self::Msg>,
                name: &str) -> Result<ActorRef<Msg>, CreateError> {
        if validate_name(name).is_ok() {
            self.kernel.create_actor(props, name, &self.myself)
        } else {
            Err(CreateError::InvalidName(name.into()))
        }
    }

    fn stop(&self, actor: &ActorRef<Self::Msg>) {
        let sys_msg = SystemMsg::ActorCmd(ActorCmd::Stop);
        actor.sys_tell(sys_msg, None);
    }
}

impl<Msg> TmpActorRefFactory for Context<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn tmp_actor_of(&self, props: BoxActorProd<Msg>) -> Result<ActorRef<Msg>, CreateError> {
        let name = rand::random::<u64>();
        let name = format!("{}", name);

        self.kernel.create_actor(props, &name, &self.system.temp_root())
    }
}

impl<Msg> ActorSelectionFactory for Context<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn select(&self, path: &str) -> Result<ActorSelection<Msg>, InvalidPath> {
        ActorSelection::new(&self.myself.clone(), path)
    }
}

impl<Msg> Timer for Context<Msg>
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

        self.kernel.schedule(Job::Repeat(job));
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

        self.kernel.schedule(Job::Once(job));
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
            Duration::from_secs(time.timestamp_millis() as u64);
        
        let id = Uuid::new_v4();
        
        let job = OnceJob {
            id: id.clone(),
            send_at: time,
            receiver: receiver,
            sender: sender,
            msg: msg.into()
        };

        self.kernel.schedule(Job::Once(job));
        id
    }

    fn cancel_schedule(&self, id: Uuid) {
        self.kernel.schedule(Job::Cancel(id));
    }
}

impl<Msg> ExecutionContext for Context<Msg>
    where Msg: Message
{
    fn execute<F: Future>(&self, f: F) -> DispatchHandle<F::Item, F::Error>
        where F: Future + Send + 'static,
                F::Item: Send + 'static,
                F::Error: Send + 'static,
    {
        self.kernel.execute(f)
    }
}

#[derive(Clone)]
pub struct Children<Msg: Message> {
    actors: Arc<RwLock<HashMap<String, ActorRef<Msg>>>>,
}

impl<Msg: Message> Children<Msg> {
    pub fn new() -> Children<Msg> {
        Children { actors: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub fn add(&self, name: &str, actor: ActorRef<Msg>) {
        self.actors.write().unwrap().insert(name.to_string(), actor);
    }

    pub fn remove(&self, name: &str) {
        self.actors.write().unwrap().remove(name);
    }

    pub fn count(&self) -> usize {
        self.iter().count()
    }

    pub fn iter(&self) -> ChildrenIterator<Msg> {
        ChildrenIterator {
            children: self,
            position: 0,
        }
    }
}

impl<'a, Msg> From<&'a ActorCell<Msg>> for Context<Msg>
    where Msg: Message
{
    fn from(cell: &ActorCell<Msg>) -> Self {
        Context {
            myself: cell.myself(),
            system: cell.inner.system.clone(),
            persistence: cell.inner.persistence.clone(),
            kernel: cell.inner.kernel.clone()
        }
    }
}

#[derive(Clone)]
pub struct ChildrenIterator<'a, Msg: Message + 'a> {
    children: &'a Children<Msg>,
    position: usize,
}

impl<'a, Msg: Message> Iterator for ChildrenIterator<'a, Msg> {
    type Item = ActorRef<Msg>;

    fn next(&mut self) -> Option<Self::Item> {
        let actors = self.children.actors.read().unwrap();
        let actor = actors.values().skip(self.position).next();
        self.position += 1;
        actor.map(|a| a.clone())
    }
}

#[derive(Clone, Debug)]
pub struct PersistenceConf {
    pub id: String,
    pub keyspace: String,
}

// #[cfg(test)]
// mod tests {
//     use actors::{Actor, ActorRefFactory, ActorSystem, Context, Envelope, Props};
//     use test_mocks::{MockModel, get_test_config};

//     struct TestActor;
//     impl Actor for TestActor {
//         type Msg = String;
//         fn receive(&mut self, _: &Context<Self::Msg>, _: Envelope<Self::Msg>) {}
//     }
    
//     #[test]
//     fn test_actor_of_bad_name() {
//         let model = MockModel;
//         let asys = ActorSystem::with_config(&model, "system", &get_test_config()).unwrap();
//         let act1 = asys.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "test").unwrap();
        
//         let ctx = act1.underlying;
//         assert!(ctx.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "@#$&*^").is_err());
//     }
    
//     #[test]
//     fn test_actor_of_already_existing() {
//         let model = MockModel;
//         let asys = ActorSystem::with_config(&model, "system", &get_test_config()).unwrap();
//         let act1 = asys.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "test").unwrap();
        
//         let ctx = act1.underlying;
//         let act1a = ctx.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "foo").unwrap();
    
//         let act1b = asys.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "test")
//             .expect_err("Duplicate actor path '/test' should fail");
    
//         let act1a2 = ctx.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "foo")
//             .expect_err("Duplicate actor path '/test/foo' should fail");
    
//         let act1c = ctx.actor_of(Props::new(Box::new(|| {Box::new(TestActor)})), "test/foo")
//             .expect_err("Duplicate actor path '/test/foo' should fail");
    
//     }
// }
