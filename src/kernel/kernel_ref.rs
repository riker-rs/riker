use std::sync::{mpsc, Arc};

use crate::{
    actor::{MsgError, MsgResult},
    kernel::{
        mailbox::{AnyEnqueueError, AnySender, MailboxSchedule, MailboxSender},
        KernelMsg,
    },
    AnyMessage, Envelope, Message,
};

#[derive(Clone)]
pub struct KernelRef {
    pub tx: mpsc::SyncSender<KernelMsg>,
}

impl KernelRef {
    pub(crate) fn schedule(&self) {
        self.send(KernelMsg::RunActor);
    }

    pub(crate) fn restart(&self) {
        self.send(KernelMsg::RestartActor);
    }

    pub(crate) fn terminate(&self) {
        self.send(KernelMsg::TerminateActor);
    }

    pub(crate) fn sys_init(&self) {
        self.send(KernelMsg::Sys);
    }

    fn send(&self, msg: KernelMsg) {
        let _ = self.tx.send(msg);
    }
}

pub fn dispatch<Msg>(
    msg: Envelope<Msg>,
    mbox: &MailboxSender<Msg>,
    kernel: &KernelRef,
) -> MsgResult<Envelope<Msg>>
where
    Msg: Message,
{
    match mbox.try_enqueue(msg) {
        Ok(_) => {
            if !mbox.is_scheduled() {
                mbox.set_scheduled(true);
                kernel.schedule();
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
) -> Result<(), AnyEnqueueError> {
    mbox.try_any_enqueue(msg, sender).map(|_| {
        if !mbox.is_sched() {
            mbox.set_sched(true);
            kernel.schedule();
        }
    })
}
