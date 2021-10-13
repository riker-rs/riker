use super::{
    kernel::KernelMsg,
    system::{ActorSystemBackend, SendingBackend},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
};

#[derive(Clone)]
pub struct ActorSystemBackendTokio {
    handle: Handle,
    s_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl From<Handle> for ActorSystemBackendTokio {
    fn from(handle: Handle) -> Self {
        ActorSystemBackendTokio {
            handle,
            s_tx: Arc::new(Mutex::new(None)),
        }
    }
}

impl ActorSystemBackend for ActorSystemBackendTokio {
    type Tx = SendingBackendTokio;

    type Rx = mpsc::Receiver<KernelMsg>;

    fn channel(&self, capacity: usize) -> (Self::Tx, Self::Rx) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            SendingBackendTokio {
                tx,
                handle: self.handle.clone(),
            },
            rx,
        )
    }

    fn spawn_receiver<F: FnMut(KernelMsg) -> bool + Send + 'static>(&self, rx: Self::Rx, mut f: F) {
        self.handle.spawn(async move {
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                if !f(msg) {
                    break;
                }
            }
        });
    }

    fn receiver_future(&self, mut rx: Self::Rx) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move { drop(rx.recv().await) })
    }
}

pub struct SendingBackendTokio {
    tx: mpsc::Sender<KernelMsg>,
    handle: Handle,
}

impl SendingBackend for SendingBackendTokio {
    fn send_msg(&self, msg: KernelMsg) {
        let tx = self.tx.clone();
        self.handle.spawn(async move { drop(tx.send(msg).await) });
    }
}
