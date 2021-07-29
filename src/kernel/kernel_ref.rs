use std::sync::Arc;

use futures::{channel::mpsc::Sender, SinkExt};

use crate::{
    actor::{MsgError, MsgResult},
    kernel::{
        mailbox::{AnyEnqueueError, AnySender, MailboxSchedule, MailboxSender},
        KernelMsg,
    },
    system::{ActorSystem, Run},
    AnyMessage, Envelope, Message,
};

#[derive(Clone)]
pub struct KernelRef {
    pub tx: Sender<KernelMsg>,
}

impl KernelRef {
    pub(crate) fn schedule(&self, sys: &ActorSystem) {
        self.send(KernelMsg::RunActor, sys);
    }

    pub(crate) fn restart(&self, sys: &ActorSystem) {
        self.send(KernelMsg::RestartActor, sys);
    }

    pub(crate) fn terminate(&self, sys: &ActorSystem) {
        self.send(KernelMsg::TerminateActor, sys);
    }

    pub(crate) fn sys_init(&self, sys: &ActorSystem) {
        self.send(KernelMsg::Sys(sys.clone()), sys);
    }

    fn send(&self, msg: KernelMsg, sys: &ActorSystem) {
        let mut tx = self.tx.clone();
        sys.run(async move {
            drop(tx.send(msg).await);
        })
        .unwrap()
        .forget();
    }
}

pub fn dispatch<Msg>(
    msg: Envelope<Msg>,
    mbox: &MailboxSender<Msg>,
    kernel: &KernelRef,
    sys: &ActorSystem,
) -> MsgResult<Envelope<Msg>>
where
    Msg: Message,
{
    match mbox.try_enqueue(msg) {
        Ok(_) => {
            if !mbox.is_scheduled() {
                mbox.set_scheduled(true);
                kernel.schedule(sys);
            }

            Ok(())
        }
        Err(e) => Err(MsgError::new(e.msg)),
    }
}

pub fn dispatch_any(
    msg: &mut AnyMessage,
    sender: crate::actor::Sender,
    mbox: &Arc<dyn AnySender>,
    kernel: &KernelRef,
    sys: &ActorSystem,
) -> Result<(), AnyEnqueueError> {
    mbox.try_any_enqueue(msg, sender).map(|_| {
        if !mbox.is_sched() {
            mbox.set_sched(true);
            kernel.schedule(sys);
        }
    })
}

unsafe impl Send for KernelRef {}
unsafe impl Sync for KernelRef {}
