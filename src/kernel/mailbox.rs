use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Deref;

use config::Config;
use log::trace;

use crate::protocol::*;
use crate::system::LogEntry;
use crate::actor::{Actor, BoxActor, ActorRef, ActorUri, ActorId};
use crate::actor::{ActorCell, Context, CellPublic, CellInternal, dead_letter, SysTell};
use crate::kernel::{KernelRef, QueueWriter, QueueReader, queue};
use crate::kernel::queue::{EnqueueResult, QueueEmpty};

#[derive(Clone)]
pub struct MailboxSender<Msg: Message> {
    pub uid: ActorId,
    queue: QueueWriter<Msg>,
    sys_queue: QueueWriter<Msg>,
    scheduled: Arc<AtomicBool>,
}

impl<Msg> MailboxSender<Msg>
    where Msg: Message {
    
    pub fn try_enqueue(&self, msg: Enqueued<Msg>) -> EnqueueResult<Msg> {
        self.queue.try_enqueue(msg)
    }

    pub fn try_sys_enqueue(&self, msg: Enqueued<Msg>) -> EnqueueResult<Msg> {
        self.sys_queue.try_enqueue(msg)
    }

    pub fn is_scheduled(&self) -> bool {
        self.scheduled.load(Ordering::Relaxed)
    }
}

unsafe impl<Msg: Message> Send for MailboxSender<Msg> {}
unsafe impl<Msg: Message> Sync for MailboxSender<Msg> {}

#[derive(Clone)]
pub struct Mailbox<Msg: Message> {
    inner: Arc<MailboxInner<Msg>>,
}

pub struct MailboxInner<Msg: Message> {
    uid: ActorId,
    msg_process_limit: u32,
    queue: QueueReader<Msg>,
    sys_queue: QueueReader<Msg>,
    kernel: KernelRef<Msg>,
    suspended: Arc<AtomicBool>,
    scheduled: Arc<AtomicBool>,
}

impl<Msg: Message> Mailbox<Msg> {
    pub fn dequeue(&self) -> Enqueued<Msg> {
        self.inner.queue.dequeue()
    }

    pub fn try_dequeue(&self) -> Result<Enqueued<Msg>, QueueEmpty> {
        self.inner.queue.try_dequeue()
    }

    pub fn sys_try_dequeue(&self) -> Result<Enqueued<Msg>, QueueEmpty> {
        self.inner.sys_queue.try_dequeue()
    }

    pub fn has_msgs(&self) -> bool {
        self.inner.queue.has_msgs()
    }

    pub fn has_sys_msgs(&self) -> bool {
        self.inner.sys_queue.has_msgs()
    }

    fn set_scheduled(&self, b: bool) {
        self.inner.scheduled.store(b, Ordering::Relaxed);
    }

    pub fn is_scheduled(&self) -> bool {
        self.inner.scheduled.load(Ordering::Relaxed)
    }

    fn set_suspended(&self, b: bool) {
        self.inner.suspended.store(b, Ordering::Relaxed);
    }

    fn is_suspended(&self) -> bool {
        self.inner.suspended.load(Ordering::Relaxed)
    }

    fn msg_process_limit(&self) -> u32 {
        self.inner.msg_process_limit
    }
}

pub trait MailboxSchedule {
    type Msg: Message;

    fn uid(&self) -> ActorId;

    fn set_scheduled(&self, b: bool);
}

impl<Msg> MailboxSchedule for MailboxSender<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn uid(&self) -> ActorId {
        self.uid
    }

    fn set_scheduled(&self, b: bool) {
        self.scheduled.store(b, Ordering::Relaxed);
    }
}

impl<Msg> MailboxSchedule for Mailbox<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn uid(&self) -> ActorId {
        self.inner.uid
    }

    fn set_scheduled(&self, b: bool) {
        self.inner.scheduled.store(b, Ordering::Relaxed);
    }
}

pub fn mailbox<Msg>(uid: ActorId,
                    msg_process_limit: u32,
                    kernel: KernelRef<Msg>)
                    -> (MailboxSender<Msg>, Mailbox<Msg>)
    where Msg: Message
{
    let (qw, qr) = queue::<Msg>();
    let (sqw, sqr) = queue::<Msg>();

    let scheduled = Arc::new(AtomicBool::new(false));

    let sender = MailboxSender {
        uid,
        queue: qw,
        sys_queue: sqw,
        scheduled: scheduled.clone()
    };

    let mailbox = MailboxInner {
        uid,
        msg_process_limit,
        queue: qr,
        sys_queue: sqr,
        kernel,
        suspended: Arc::new(AtomicBool::new(true)), //todo this can't be a bool?
        scheduled
    };

    let mailbox = Mailbox {
        inner: Arc::new(mailbox)
    };

    (sender, mailbox)
}

pub fn run_mailbox<Msg>(mbox: Mailbox<Msg>,
                        cell: ActorCell<Msg>,
                        mut actor: Option<BoxActor<Msg>>)
    where Msg: Message
{
    let c = &cell;
    let ctx: Context<Msg> = c.into();

    process_sys_msgs(&mbox, &cell, &ctx, &mut actor);

    if actor.is_some() && !mbox.is_suspended() {
        process_msgs(&mbox, &cell, &ctx, &mut actor);
    }

    mbox.inner.kernel.park_actor(mbox.inner.uid, actor);
    mbox.set_scheduled(false);

    if (mbox.has_msgs() && !mbox.is_suspended() && !mbox.is_scheduled())|| mbox.has_sys_msgs() {
        mbox.inner.kernel.schedule_actor(&mbox);
    }
}

fn process_msgs<Msg>(mbox: &Mailbox<Msg>,
                    cell: &ActorCell<Msg>,
                    ctx: &Context<Msg>,
                    actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    let mut count = 0;
    
    let _sentinel = Sentinel {
        parent: cell.parent(),
        actor: cell.myself(),
        mbox: mbox.clone(),
    };

    loop {
        if count < mbox.msg_process_limit() && !cell.is_persisting() {
            match mbox.try_dequeue() {
                Ok(msg) => {
                    handle_msg(msg, cell, &ctx, actor, mbox);
                    process_sys_msgs(mbox, cell, &ctx, actor);
                    count +=1;
                },
                Err(_) => {
                    break;
                }
            }
        } else {
            break;
        }
    }
}

fn process_sys_msgs<Msg>(mbox: &Mailbox<Msg>,
                        cell: &ActorCell<Msg>,
                        ctx: &Context<Msg>,
                        actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    // All system messages are processed in this mailbox execution
    // and we prevent any new messages that have since been added to the queue
    // from being processed by staging them in a Vec.
    // This prevents during actor restart.
    let mut sys_messages: Vec<Enqueued<Msg>> = Vec::new();
    loop {
        match mbox.sys_try_dequeue() {
            Ok(sys_msg) => {
                sys_messages.push(sys_msg);
            }
            Err(_) => break
        }
    }

    for sys_msg in sys_messages.into_iter() {
        handle_msg(sys_msg, cell, ctx, actor, mbox);
    }
}

pub fn flush_to_deadletters<Msg>(mbox: &Mailbox<Msg>,
                                dl: &ActorRef<Msg>,
                                uri: &ActorUri)
    where Msg: Message
{
    loop {
        match mbox.try_dequeue() {
            Ok(msg) => {
                match msg {
                    Enqueued::ActorMsg(am) => {
                        if let ActorMsg::User(_) = am.msg {
                            // TODO candidate for improving code readability
                            let sp = am.sender.clone().map(|s| s.uri.path.deref().clone());
                            let mp = uri.path.deref().clone();
                            dead_letter(dl,
                                        sp,
                                        mp,
                                        am.msg);
                        }
                    }
                    _ => {} // TODO handle system messages?
                }
            },
            Err(_) => {
                break;
            }
        }
    }
}

fn handle_msg<Msg>(msg: Enqueued<Msg>,
                    cell: &ActorCell<Msg>,
                    ctx: &Context<Msg>,
                    actor: &mut Option<BoxActor<Msg>>,
                    mbox: &Mailbox<Msg>)
    where Msg: Message
{
    match msg {
        Enqueued::ActorMsg(envelope) => {
            if actor.is_some() {
                handle_actor_msg(envelope, cell, ctx, actor);
            }
        }
        Enqueued::SystemMsg(envelope) => handle_sys_msg(envelope, cell, ctx, actor, mbox)
    }
}

fn handle_actor_msg<Msg>(msg: Envelope<Msg>,
                        cell: &ActorCell<Msg>,
                        ctx: &Context<Msg>, 
                        actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    match (msg.msg, msg.sender) {
        (ActorMsg::User(msg), sender) => actor.as_mut().unwrap().receive(ctx, msg, sender),
        (ActorMsg::Identify, sender) => handle_identify(sender, cell),
        (msg, sender) => actor.as_mut().unwrap().other_receive(ctx, msg, sender)
    }
}

fn handle_sys_msg<Msg>(msg: SystemEnvelope<Msg>,
                        cell: &ActorCell<Msg>,
                        ctx: &Context<Msg>,
                        actor: &mut Option<BoxActor<Msg>>,
                        mbox: &Mailbox<Msg>)
    where Msg: Message
{
    match msg.msg {
        SystemMsg::ActorInit => handle_init(cell, ctx, actor, mbox),
        SystemMsg::ActorCmd(cmd) => handle_cmd(cmd, cell, actor),
        SystemMsg::Event(ref evt) => handle_evt(evt.clone(), msg.clone(), cell, ctx, actor),
        SystemMsg::Failed(failed) => handle_failed(failed, cell, actor),
        SystemMsg::Persisted(evt, sender) => handle_persisted(evt, cell, ctx, actor, sender),
        SystemMsg::Replay(evts) => handle_replay(evts, cell, ctx, actor, mbox),
        SystemMsg::Log(entry) => handle_log_msg(entry, ctx, actor),
    }
}

fn handle_init<Msg>(cell: &ActorCell<Msg>,
                    ctx: &Context<Msg>,
                    actor: &mut Option<BoxActor<Msg>>,
                    mbox: &Mailbox<Msg>)
    where Msg: Message
{
    trace!("ACTOR INIT");
    actor.as_mut().unwrap().pre_start(ctx);

    // todo the intent here can be made clearer
    // if persistence is not configured then set as not suspended
    if cell.load_events(actor) {
        actor.as_mut().unwrap().post_start(ctx);
        mbox.set_suspended(false);
    }
}

fn handle_identify<Msg>(sender: Option<ActorRef<Msg>>,
                        cell: &ActorCell<Msg>)
    where Msg: Message
{
    trace!("ACTOR IDENTIFY");
    cell.identify(sender);
}

fn handle_cmd<Msg>(cmd: ActorCmd,
                    cell: &ActorCell<Msg>,
                    actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    // trace!("{}: ACTOR CMD {:?}", cell.myself().uri.path(), cmd);
    cell.receive_cmd(cmd, actor);
}

fn handle_evt<Msg>(evt: SystemEvent<Msg>,
                    msg: SystemEnvelope<Msg>,
                    cell: &ActorCell<Msg>,
                    ctx: &Context<Msg>,
                    actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    // trace!("{} ACTOR EVT {:?}", cell.into().path(), evt);
    if actor.is_some() {
        actor.as_mut().unwrap().system_receive(ctx, msg.msg, msg.sender);
    }
    
    if let SystemEvent::ActorTerminated(ref actor_ref) = evt {
        cell.death_watch(actor_ref, actor);
    }
}

fn handle_failed<Msg>(failed: ActorRef<Msg>,
                        cell: &ActorCell<Msg>,
                        actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    trace!("ACTOR HANDLE FAILED");
    cell.handle_failure(failed, actor.as_mut().unwrap().supervisor_strategy())
}

fn handle_persisted<Msg>(evt: Msg,
                        cell: &ActorCell<Msg>,
                        ctx: &Context<Msg>,
                        actor: &mut Option<BoxActor<Msg>>,
                        sender: Option<ActorRef<Msg>>)
    where Msg: Message
{
    trace!("ACTOR HANDLE PERSISTED");
    cell.set_persisting(false);
    actor.as_mut().unwrap().apply_event(ctx, evt, sender);
}

fn handle_replay<Msg>(evts: Vec<Msg>,
                        cell: &ActorCell<Msg>,
                        ctx: &Context<Msg>,
                        actor: &mut Option<BoxActor<Msg>>,
                        mbox: &Mailbox<Msg>)
    where Msg: Message
{
    trace!("ACTOR REPLAY");

    cell.replay(ctx, evts, actor);

    actor.as_mut().unwrap().post_start(ctx);
    mbox.set_suspended(false);
}

fn handle_log_msg<Msg>(entry: LogEntry,
                        ctx: &Context<Msg>,
                        actor: &mut Option<BoxActor<Msg>>)
    where Msg: Message
{
    if actor.is_some() {
        actor.as_mut().unwrap().system_receive(ctx, SystemMsg::Log(entry), None);
    }
}

struct Sentinel<Msg: Message> {
    parent: ActorRef<Msg>,
    actor: ActorRef<Msg>,
    mbox: Mailbox<Msg>,
}

impl<Msg> Drop for Sentinel<Msg>
    where Msg: Message
{
    fn drop(&mut self) {
        if thread::panicking() {
            // Suspend the mailbox to prevent further message processing
            self.mbox.set_suspended(true);

            // There is no actor to park but kernel still needs to mark as no longer scheduled
            // self.kernel.park_actor(self.actor.uri.uid, None);
            self.mbox.set_scheduled(false);

            // Message the parent (this failed actor's supervisor) to decide how to handle the failure
            self.parent.sys_tell(SystemMsg::Failed(self.actor.clone()), None);
        }
    }
}

#[derive(Clone, Debug)]
pub struct MailboxConfig {
    pub msg_process_limit: u32,
}

impl<'a> From<&'a Config> for MailboxConfig {
    fn from(cfg: &Config) -> Self {
        MailboxConfig {
            msg_process_limit: cfg.get_int("mailbox.msg_process_limit").unwrap() as u32
        }
    }
}
