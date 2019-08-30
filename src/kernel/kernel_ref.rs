use std::sync::Arc;

use futures::{channel::mpsc::Sender, SinkExt};

use crate::{
    actor::{MsgError, MsgResult},
    kernel::{
        mailbox::{AnySender, MailboxSchedule, MailboxSender},
        KernelMsg,
    },
    system::ActorSystem,
    AnyMessage, Envelope, Message,
};

#[derive(Clone)]
pub struct KernelRef {
    pub tx: Sender<KernelMsg>,
}

impl KernelRef {
    pub(crate) async fn schedule(&self) {
        self.send(KernelMsg::RunActor).await;
    }

    pub(crate) async fn restart(&self) {
        self.send(KernelMsg::RestartActor).await;
    }

    pub(crate) async fn terminate(&self) {
        self.send(KernelMsg::TerminateActor).await;
    }

    pub(crate) async fn sys_init(&self, sys: &ActorSystem) {
        self.send(KernelMsg::Sys(sys.clone())).await;
    }

    async fn send(&self, msg: KernelMsg) {
        let mut tx = self.tx.clone();
        tx.send(msg).await.unwrap();
    }
}

pub async fn dispatch<Msg>(
    msg: Envelope<Msg>,
    mbox: &MailboxSender<Msg>,
    kernel: &KernelRef,
) -> MsgResult<Envelope<Msg>>
where
    Msg: Message,
{
    match mbox.try_enqueue(msg).await {
        Ok(_) => {
            if !mbox.is_scheduled() {
                mbox.set_scheduled(true);
                kernel.schedule().await;
            }

            Ok(())
        }
        Err(e) => Err(MsgError::new(e.msg.into())),
    }
}

pub async fn dispatch_any(
    msg: &mut AnyMessage,
    sender: crate::actor::Sender,
    mbox: &Arc<dyn AnySender>,
    kernel: &KernelRef,
) -> Result<(), ()> {
    match mbox.try_any_enqueue(msg, sender).await {
        Ok(_) => {
            if !mbox.is_sched() {
                mbox.set_sched(true);
                kernel.schedule().await;
            }

            Ok(())
        }
        Err(_) => Err(()),
    }
}

