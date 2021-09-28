use std::sync::{Arc, Mutex};
use tokio::{runtime::Handle, sync::{oneshot, mpsc}};
use super::{
    system::{ActorSystemBackend, SendingBackend},
    kernel::KernelMsg,
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

    type ShutdownFuture = oneshot::Receiver<()>;

    fn channel(&self, capacity: usize) -> (Self::Tx, Self::Rx) {
        let (tx, rx) = mpsc::channel(capacity);
        (SendingBackendTokio { tx, handle: self.handle.clone() }, rx)
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

    fn get_shutdown_future(&self) -> Self::ShutdownFuture {
        let (tx, rx) = oneshot::channel();
        *self.s_tx.lock().unwrap() = Some(tx);
        rx
    }

    fn shutdown(&self) {
        if let Some(tx) = self.s_tx.lock().unwrap().take() {
            tx.send(()).unwrap();
        }
    }
}

pub struct SendingBackendTokio {
    tx: mpsc::Sender<KernelMsg>,
    handle: Handle,
}

impl SendingBackend for SendingBackendTokio {
    fn send_msg(&self, msg: KernelMsg) {
        let tx = self.tx.clone();
        let _ = self.handle.spawn(async move { drop(tx.send(msg).await) });
    }
}
