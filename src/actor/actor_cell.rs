use std::{
    collections::HashMap,
    fmt,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::{
    actor::{props::ActorFactory, *},
    kernel::{
        kernel_ref::{dispatch, dispatch_any, KernelRef},
        mailbox::{AnyEnqueueError, AnySender, MailboxSender},
    },
    system::{
        timer::{Job, OnceJob, RepeatJob, ScheduleId, Timer},
        ActorSystem, SystemCmd, SystemMsg,
    },
    AnyMessage, Envelope, Message,
};

#[derive(Clone)]
pub struct ActorCell {
    inner: Arc<ActorCellInner>,
}

#[derive(Clone)]
struct ActorCellInner {
    uri: ActorUri,
    parent: Option<BasicActorRef>,
    children: Children,
    is_remote: bool,
    is_terminating: Arc<AtomicBool>,
    is_restarting: Arc<AtomicBool>,
    status: Arc<AtomicUsize>,
    kernel: Option<KernelRef>,
    system: ActorSystem,
    mailbox: Arc<dyn AnySender>,
    sys_mailbox: MailboxSender<SystemMsg>,
}

impl ActorCell {
    /// Constructs a new `ActorCell`
    pub(crate) fn new(
        uri: ActorUri,
        parent: Option<BasicActorRef>,
        system: &ActorSystem,
        mailbox: Arc<dyn AnySender>,
        sys_mailbox: MailboxSender<SystemMsg>,
    ) -> ActorCell {
        ActorCell {
            inner: Arc::new(ActorCellInner {
                uri,
                parent,
                children: Children::new(),
                is_remote: false,
                is_terminating: Arc::new(AtomicBool::new(false)),
                is_restarting: Arc::new(AtomicBool::new(false)),
                status: Arc::new(AtomicUsize::new(0)),
                kernel: None,
                system: system.clone(),
                mailbox,
                sys_mailbox,
            }),
        }
    }

    pub(crate) fn init(self, kernel: &KernelRef) -> ActorCell {
        let inner = ActorCellInner {
            kernel: Some(kernel.clone()),
            ..self.inner.deref().clone()
        };

        ActorCell {
            inner: Arc::new(inner),
        }
    }

    pub(crate) fn kernel(&self) -> &KernelRef {
        self.inner.kernel.as_ref().unwrap()
    }

    pub(crate) fn myself(&self) -> BasicActorRef {
        BasicActorRef { cell: self.clone() }
    }

    pub(crate) fn uri(&self) -> &ActorUri {
        &self.inner.uri
    }

    pub(crate) fn parent(&self) -> BasicActorRef {
        self.inner.parent.as_ref().unwrap().clone()
    }

    pub fn has_children(&self) -> bool {
        self.inner.children.len() > 0
    }

    pub(crate) fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        Box::new(self.inner.children.iter())
    }

    pub(crate) fn user_root(&self) -> BasicActorRef {
        self.inner.system.user_root().clone()
    }

    pub(crate) fn is_root(&self) -> bool {
        self.myself().path() == "/"
    }

    pub fn is_user(&self) -> bool {
        self.inner.system.user_root().is_child(&self.myself())
    }

    pub(crate) fn send_any_msg(
        &self,
        msg: &mut AnyMessage,
        sender: crate::actor::Sender,
    ) -> Result<(), AnyEnqueueError> {
        let mb = &self.inner.mailbox;
        let k = self.kernel();

        dispatch_any(msg, sender, mb, k)
    }

    pub(crate) fn send_sys_msg(&self, msg: Envelope<SystemMsg>) -> MsgResult<Envelope<SystemMsg>> {
        let mb = &self.inner.sys_mailbox;

        let k = self.kernel();
        dispatch(msg, mb, k)
    }

    pub(crate) fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.inner.children.any(actor)
    }

    pub(crate) fn stop(&self, actor: &BasicActorRef) {
        actor.sys_tell(SystemCmd::Stop.into());
    }

    pub fn add_child(&self, actor: BasicActorRef) {
        self.inner.children.add(actor);
    }

    /// return true if there is no children left
    pub fn remove_child_is_empty(&self, actor: &BasicActorRef) -> bool {
        self.inner.children.remove(actor)
    }

    pub fn receive_cmd<A: Actor>(&self, cmd: SystemCmd, actor: &mut Option<A>) {
        match cmd {
            SystemCmd::Stop => self.terminate(actor),
            SystemCmd::Restart => self.restart(),
        }
    }

    pub fn terminate<A: Actor>(&self, actor: &mut Option<A>) {
        // *1. Suspend non-system mailbox messages
        // *2. Iterate all children and send Stop to each
        // *3. Wait for ActorTerminated from each child

        self.inner.is_terminating.store(true, Ordering::Relaxed);

        if !self.has_children() {
            self.kernel().terminate();
            post_stop(actor);
        } else {
            self.inner.children.for_each(|child| self.stop(child));
        }
    }

    pub fn restart(&self) {
        if !self.has_children() {
            self.kernel().restart();
        } else {
            self.inner.is_restarting.store(true, Ordering::Relaxed);
            self.inner.children.for_each(|child| self.stop(child));
        }
    }

    pub fn death_watch<A: Actor>(&self, terminated: &BasicActorRef, actor: &mut Option<A>) {
        if self.remove_child_is_empty(terminated) {
            // No children exist. Stop this actor's kernel.
            if self.inner.is_terminating.load(Ordering::Relaxed) {
                self.kernel().terminate();
                post_stop(actor);
            }

            // No children exist. Restart the actor.
            if self.inner.is_restarting.load(Ordering::Relaxed) {
                self.inner.is_restarting.store(false, Ordering::Relaxed);
                self.kernel().restart();
            }
        }
    }

    pub fn handle_failure(&self, failed: BasicActorRef) {
        self.restart_child(&failed)
    }

    pub fn restart_child(&self, actor: &BasicActorRef) {
        actor.sys_tell(SystemCmd::Restart.into());
    }

    pub fn escalate_failure(&self) {
        self.inner
            .parent
            .as_ref()
            .unwrap()
            .sys_tell(SystemMsg::Failed(self.myself()));
    }
}

impl<Msg: Message> From<ExtendedCell<Msg>> for ActorCell {
    fn from(cell: ExtendedCell<Msg>) -> Self {
        cell.cell
    }
}

impl fmt::Debug for ActorCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorCell[{:?}]", self.uri())
    }
}

#[derive(Clone)]
pub struct ExtendedCell<Msg: Message> {
    cell: ActorCell,
    mailbox: MailboxSender<Msg>,
}

impl<Msg> ExtendedCell<Msg>
where
    Msg: Message,
{
    pub(crate) fn new(
        uri: ActorUri,
        parent: Option<BasicActorRef>,
        system: &ActorSystem,
        any_mailbox: Arc<dyn AnySender>,
        sys_mailbox: MailboxSender<SystemMsg>,
        mailbox: MailboxSender<Msg>,
    ) -> Self {
        let cell = ActorCell {
            inner: Arc::new(ActorCellInner {
                uri,
                parent,
                children: Children::new(),
                is_remote: false,
                is_terminating: Arc::new(AtomicBool::new(false)),
                is_restarting: Arc::new(AtomicBool::new(false)),
                status: Arc::new(AtomicUsize::new(0)),
                kernel: None,
                system: system.clone(),
                mailbox: any_mailbox,
                sys_mailbox,
            }),
        };

        ExtendedCell { cell, mailbox }
    }

    pub(crate) fn init(self, kernel: &KernelRef) -> Self {
        let cell = self.cell.init(kernel);

        ExtendedCell { cell, ..self }
    }

    pub fn myself(&self) -> ActorRef<Msg> {
        self.cell.myself().typed(self.clone())
    }

    pub fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    pub fn parent(&self) -> BasicActorRef {
        self.cell.parent()
    }

    pub fn has_children(&self) -> bool {
        self.cell.has_children()
    }

    pub(crate) fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.cell.is_child(actor)
    }

    pub fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    pub fn user_root(&self) -> BasicActorRef {
        self.cell.user_root()
    }

    pub fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    pub fn is_user(&self) -> bool {
        self.cell.is_user()
    }

    pub(crate) fn send_msg(&self, msg: Envelope<Msg>) -> MsgResult<Envelope<Msg>> {
        let mb = &self.mailbox;
        let k = self.cell.kernel();

        dispatch(msg, mb, k).map_err(|e| {
            let dl = e.clone(); // clone the failed message and send to dead letters
            let dl = DeadLetter {
                msg: format!("{:?}", dl.msg.msg),
                sender: dl.msg.sender,
                recipient: self.cell.myself(),
            };

            self.cell.inner.system.dead_letters().tell(
                Publish {
                    topic: "dead_letter".into(),
                    msg: dl,
                },
                None,
            );

            e
        })
    }

    pub(crate) fn send_sys_msg(&self, msg: Envelope<SystemMsg>) -> MsgResult<Envelope<SystemMsg>> {
        self.cell.send_sys_msg(msg)
    }

    pub fn system(&self) -> &ActorSystem {
        &self.cell.inner.system
    }

    pub(crate) fn handle_failure(&self, failed: BasicActorRef) {
        self.cell.handle_failure(failed)
    }

    pub(crate) fn receive_cmd<A: Actor>(&self, cmd: SystemCmd, actor: &mut Option<A>) {
        self.cell.receive_cmd(cmd, actor)
    }

    pub(crate) fn death_watch<A: Actor>(&self, terminated: &BasicActorRef, actor: &mut Option<A>) {
        self.cell.death_watch(terminated, actor)
    }
}

impl<Msg: Message> fmt::Debug for ExtendedCell<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtendedCell[{:?}]", self.uri())
    }
}

fn post_stop<A: Actor>(actor: &mut Option<A>) {
    // If the actor instance exists we can execute post_stop.
    // The instance will be None if this is an actor that has failed
    // and is being terminated by an escalated supervisor.
    if let Some(act) = actor.as_mut() {
        act.post_stop();
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
/// actor within the heirarchy.
///
/// Since `Context` is specific to an actor and its functions
/// it is not cloneable.
pub struct Context<Msg: Message> {
    pub myself: ActorRef<Msg>,
    pub system: ActorSystem,
    pub(crate) kernel: KernelRef,
}

impl<Msg> Context<Msg>
where
    Msg: Message,
{
    /// Returns the `ActorRef` of the current actor.
    pub fn myself(&self) -> ActorRef<Msg> {
        self.myself.clone()
    }
}

impl<Msg: Message> ActorRefFactory for Context<Msg> {
    fn actor_of_props<A>(
        &self,
        name: &str,
        props: BoxActorProd<A>,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor,
    {
        self.system
            .provider
            .create_actor(props, name, &self.myself().into(), &self.system)
    }

    fn actor_of<A>(&self, name: &str) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
    where
        A: ActorFactory,
    {
        self.system.provider.create_actor(
            Props::new::<A>(),
            name,
            &self.myself().into(),
            &self.system,
        )
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
        self.system.provider.create_actor(
            Props::new_args::<A, _>(args),
            name,
            &self.myself().into(),
            &self.system,
        )
    }

    fn stop(&self, actor: impl ActorReference) {
        actor.sys_tell(SystemCmd::Stop.into());
    }
}

impl<Msg> Timer for Context<Msg>
where
    Msg: Message,
{
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

        let _ = self.system.timer.lock().unwrap().send(Job::Repeat(job));
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

        let _ = self.system.timer.lock().unwrap().send(Job::Once(job));
        id
    }

    fn cancel_schedule(&self, id: Uuid) {
        let _ = self.system.timer.lock().unwrap().send(Job::Cancel(id));
    }
}

#[derive(Clone)]
pub struct Children {
    actors: Arc<RwLock<HashMap<String, BasicActorRef>>>,
}

impl Children {
    pub fn new() -> Children {
        Children {
            actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add(&self, actor: BasicActorRef) {
        self.actors
            .write()
            .unwrap()
            .insert(actor.name().to_string(), actor);
    }

    pub fn remove(&self, actor: &BasicActorRef) -> bool {
        let mut g = self.actors.write().unwrap();
        g.remove(actor.name());
        g.is_empty()
    }

    pub fn len(&self) -> usize {
        self.actors.read().unwrap().len()
    }

    pub fn for_each<F>(&self, f: F)
    where
        F: FnMut(&BasicActorRef),
    {
        self.actors.read().unwrap().values().for_each(f)
    }

    pub fn any(&self, actor: &BasicActorRef) -> bool {
        self.actors
            .read()
            .unwrap()
            .values()
            .any(|child| *child == *actor)
    }

    pub fn iter(&self) -> impl Iterator<Item = BasicActorRef> {
        self.actors
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
    }
}
