use std::sync::Arc;

use futures::{channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender}, lock::Mutex, prelude::*};

use crate::{Envelope, Message};

pub fn queue<Msg: Message>() -> (QueueWriter<Msg>, QueueReader<Msg>) {
    let (tx, rx) = unbounded::<Envelope<Msg>>();

    let qw = QueueWriter { tx: Arc::new(Mutex::new(tx)) };

    let qr = QueueReaderInner {
        rx,
        next_item: None,
    };

    let qr = QueueReader {
        inner: Mutex::new(qr),
    };

    (qw, qr)
}

#[derive(Clone)]
pub struct QueueWriter<Msg: Message> {
    tx: Arc<Mutex<UnboundedSender<Envelope<Msg>>>>,
}

impl<Msg: Message> QueueWriter<Msg> {
    pub async fn try_enqueue(&self, msg: Envelope<Msg>) -> EnqueueResult<Msg> {
        let mut tx = self.tx.lock().await;
        tx.send(msg.clone())
            .await
            .map_err(|_| EnqueueError { msg })
    }
}

pub struct QueueReader<Msg: Message> {
    inner: Mutex<QueueReaderInner<Msg>>,
}

struct QueueReaderInner<Msg: Message> {
    rx: UnboundedReceiver<Envelope<Msg>>,
    next_item: Option<Envelope<Msg>>,
}

impl<Msg: Message> QueueReader<Msg> {
    #[allow(dead_code)]
    pub async fn dequeue(&self) -> Envelope<Msg> {
        let mut inner = self.inner.lock().await;
        if let Some(item) = inner.next_item.take() {
            item
        } else {
            inner.rx.next().await.unwrap()
        }
    }

    pub async fn try_dequeue(&self) -> DequeueResult<Envelope<Msg>> {
        let mut inner = self.inner.lock().await;
        if let Some(item) = inner.next_item.take() {
            Ok(item)
        } else {
            inner.rx
                .try_next()
                .map(|v| v.unwrap())
                .map_err(|_| QueueEmpty)
        }
    }

    pub async fn has_msgs(&self) -> bool {
        let mut inner = self.inner.lock().await;
        inner.next_item.is_some() || {
            match inner.rx.try_next() {
                Ok(Some(item)) => {
                    inner.next_item = Some(item);
                    true
                }
                Ok(None)
                | Err(_) => false,
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnqueueError<T> {
    pub msg: T,
}

pub type EnqueueResult<Msg> = Result<(), EnqueueError<Envelope<Msg>>>;

pub struct QueueEmpty;
pub type DequeueResult<Msg> = Result<Msg, QueueEmpty>;
