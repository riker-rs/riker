use futures::{channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender}, lock::Mutex, prelude::*};

use crate::{Envelope, Message};

pub fn queue<Msg: Message>() -> (QueueWriter<Msg>, QueueReader<Msg>) {
    let (tx, rx) = unbounded::<Envelope<Msg>>();

    let qw = QueueWriter { tx };

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
    tx: UnboundedSender<Envelope<Msg>>,
}

impl<Msg: Message> QueueWriter<Msg> {
    pub async fn try_enqueue(&self, msg: Envelope<Msg>) -> EnqueueResult<Msg> {
        let mut tx = self.tx.clone();
        tx.send(msg.clone())
            .await
            .map_err(|_| EnqueueError { msg })
    }
}

pub struct QueueReader<Msg: Message> {
    inner: Mutex<QueueReaderInner<Msg>>,
}

struct QueueReaderInner<Msg: Message> {
    /// Receiving channel
    rx: UnboundedReceiver<Envelope<Msg>>,
    /// Here will be value stored in case `self.rx` contained Some value
    /// when the method `Self::has_msgs()` was called.
    next_item: Option<Envelope<Msg>>,
}

impl<Msg: Message> QueueReader<Msg> {
    #[allow(dead_code)]
    pub async fn dequeue(&self) -> Envelope<Msg> {
        let mut inner = self.inner.lock().await;
        if let Some(item) = inner.next_item.take() {
            item
        } else {
            inner.rx.next().await.expect("Cannot dequeue empty queue")
        }
    }

    pub async fn try_dequeue(&self) -> DequeueResult<Envelope<Msg>> {
        let mut inner = self.inner.lock().await;
        // try to take (take == item will be None on next call) item from "next_item"
        if let Some(item) = inner.next_item.take() {
            // item was Some so return it's value
            Ok(item)
        } else {
            // try to receive value from "rx"
            match inner.rx.try_next() {
                Ok(Some(item)) => Ok(item),             // found some value
                Err(_) | Ok(None) => Err(QueueEmpty)    // found no value or channel was closed
            }
        }
    }

    pub async fn has_msgs(&self) -> bool {
        let mut inner = self.inner.lock().await;
        inner.next_item.is_some() || {
            match inner.rx.try_next() {
                Ok(Some(item)) => {
                    // store received value for later use
                    inner.next_item = Some(item);
                    true
                }
                Err(_) | Ok(None) => false,
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
