use std::sync::mpsc::{channel, Sender};

use futures::Future;
use futures::channel::oneshot;

use crate::protocol::{Message, Envelope, SystemEnvelope, Enqueued};
use crate::actor::{BoxActor, ActorRef, ActorId, BoxActorProd};
use crate::actor::{CreateError, MsgError, MsgResult};
use crate::futures_util::{MySender, DispatchHandle};
use crate::kernel::{KernelMsg, MailboxSender, MailboxSchedule};
use crate::system::Job;

use self::KernelMsg::{CreateActor, RestartActor, TerminateActor};
use self::KernelMsg::{ParkActor, UnparkActor, Stop, RunFuture};

#[derive(Clone)]
pub struct KernelRef<Msg: Message> {
    pub kernel_tx: Sender<KernelMsg<Msg>>,
    pub timer_tx: Sender<Job<Msg>>,
}

impl<Msg> KernelRef<Msg>
    where Msg: Message
{
    pub fn dispatch(&self,
                    msg: Envelope<Msg>,
                    mbox: &MailboxSender<Msg>) -> MsgResult<Envelope<Msg>> {

        let msg = Enqueued::ActorMsg(msg);
        match mbox.try_enqueue(msg) {
            Ok(_) => {
                if !mbox.is_scheduled() {
                    self.schedule_actor(mbox);
                }
                
                Ok(())
            }
            Err(e) => Err(MsgError::new(e.msg.into()))
        }
    }

    pub fn dispatch_sys(&self,
                        msg: SystemEnvelope<Msg>,
                        mbox: &MailboxSender<Msg>) -> MsgResult<SystemEnvelope<Msg>> {
        let msg = Enqueued::SystemMsg(msg);
        match mbox.try_sys_enqueue(msg) {
            Ok(_) => {
                if !mbox.is_scheduled() {
                    self.schedule_actor(mbox);
                }
                Ok(())
            }
            Err(e) => Err(MsgError::new(e.msg.into()))
        }
    }

    pub fn schedule_actor<Mbs>(&self, mbox: &Mbs)
        where Mbs: MailboxSchedule<Msg=Msg>
    {
        mbox.set_scheduled(true);
        send(UnparkActor(mbox.uid()), &self.kernel_tx);
    }

    pub fn create_actor(&self,
                        props: BoxActorProd<Msg>,
                        name: &str,
                        parent: &ActorRef<Msg>) -> Result<ActorRef<Msg>, CreateError> {

        let (tx, rx) = channel();
        let msg = CreateActor(props,
                                name.to_string(),
                                parent.clone(),
                                tx);
        send(msg, &self.kernel_tx);

        rx.recv().unwrap()
    }

    pub fn park_actor(&self, uid: ActorId, actor: Option<BoxActor<Msg>>) {
        send(ParkActor(uid, actor), &self.kernel_tx);
    }

    pub fn terminate_actor(&self, uid: ActorId) {
        send(TerminateActor(uid), &self.kernel_tx);
    }

    pub fn restart_actor(&self, uid: ActorId) {
        send(RestartActor(uid), &self.kernel_tx);
    }

    pub fn stop_kernel(&self) {
        send(Stop, &self.kernel_tx);
    }

    pub fn execute<F: Future>(&self, f: F) -> DispatchHandle<F::Item, F::Error>
        where F: Future + Send + 'static,
                F::Item: Send + 'static,
                F::Error: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        let sender = MySender {
            fut: f,
            tx: Some(tx),
        };
    
        send(RunFuture(Box::new(sender)), &self.kernel_tx);

        DispatchHandle {
            inner: rx
        }
    }

    // Scheduler

    pub fn schedule(&self, job: Job<Msg>) {
        drop(self.timer_tx.send(job))
    }
}

fn send<Msg>(msg: KernelMsg<Msg>, tx: &Sender<KernelMsg<Msg>>)
    where Msg: Message
{
    drop(tx.send(msg))
}

unsafe impl<Msg: Message> Send for KernelRef<Msg> {}
unsafe impl<Msg: Message> Sync for KernelRef<Msg> {}

// This exists as a temporary solution to allow getting the original msg
// out of the Enqueued wrapper.
// Instead the mailbox and queue functions should be refactored
// to not require Enqueued
impl<Msg: Message> Into<Envelope<Msg>> for Enqueued<Msg> {
    fn into(self) -> Envelope<Msg> {
        match self {
            Enqueued::ActorMsg(msg) => msg,
            _ => panic!("")

        }
    }
}

// This exists as a temporary solution to allow getting the original msg
// out of the Enqueued wrapper.
// Instead the mailbox and queue functions should be refactored
// to not require Enqueued
impl<Msg: Message> Into<SystemEnvelope<Msg>> for Enqueued<Msg> {
    fn into(self) -> SystemEnvelope<Msg> {
        match self {
            Enqueued::SystemMsg(msg) => msg,
            _ => panic!("")

        }
    }
}
