use std::sync::Mutex;
use flume::{Sender, Receiver};

use crate::{Envelope, Message};

pub fn queue<Msg: Message>() -> (QueueWriter<Msg>, QueueReader<Msg>) {
    let (tx, rx) = flume::unbounded::<Envelope<Msg>>();

    let qw = QueueWriter { tx };

    let qr = QueueReader {
        rx,
        next_item: Mutex::new(None),
    };

    (qw, qr)
}

#[derive(Clone)]
pub struct QueueWriter<Msg: Message> {
    tx: Sender<Envelope<Msg>>,
}

impl<Msg: Message> QueueWriter<Msg> {
    pub fn try_enqueue(&self, msg: Envelope<Msg>) -> EnqueueResult<Msg> {
        self.tx
            .send(msg)
            .map(|_| ())
            .map_err(|e| EnqueueError { msg: e.0 })
    }
}

pub struct QueueReader<Msg: Message> {
    rx: Receiver<Envelope<Msg>>,
    next_item: Mutex<Option<Envelope<Msg>>>,
}

impl<Msg: Message> QueueReader<Msg> {
    #[allow(dead_code)]
    pub fn dequeue(&self) -> Envelope<Msg> {
        let mut next_item = self.next_item.lock().unwrap();
        if let Some(item) = next_item.take() {
            item
        } else {
            drop(next_item);
            self.rx.recv().unwrap()
        }
    }

    pub fn try_dequeue(&self) -> DequeueResult<Envelope<Msg>> {
        let mut next_item = self.next_item.lock().unwrap();
        if let Some(item) = next_item.take() {
            Ok(item)
        } else {
            drop(next_item);
            self.rx.try_recv().map_err(|_| QueueEmpty)
        }
    }

    pub fn has_msgs(&self) -> bool {
        let mut next_item = self.next_item.lock().unwrap();
        next_item.is_some() || {
            match self.rx.try_recv() {
                Ok(item) => {
                    *next_item = Some(item);
                    true
                }
                Err(_) => false,
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
