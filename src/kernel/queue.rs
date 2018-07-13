use std::sync::Mutex;
use std::sync::mpsc::{channel, Sender, Receiver};

use protocol::{Message, Enqueued};

pub fn queue<Msg: Message>() -> (QueueWriter<Msg>, QueueReader<Msg>) {
    let (tx, rx) = channel::<Enqueued<Msg>>();
   
    let qw = QueueWriter {
        tx: tx,
    };

    let qr = QueueReaderInner {
        rx: rx,
        next_item: None,
    };

    let qr = QueueReader {
        inner: Mutex::new(qr)
    };

    (qw, qr)
}

#[derive(Clone)]
pub struct QueueWriter<Msg: Message> {
    tx: Sender<Enqueued<Msg>>,
}

impl<Msg: Message> QueueWriter<Msg> {
    pub fn try_enqueue(&self, msg: Enqueued<Msg>) -> EnqueueResult<Msg> {
        self.tx.send(msg)
            .map(|_| ())
            .map_err(|e| EnqueueError { msg: e.0 })
    }
}

pub struct QueueReader<Msg: Message> {
    inner: Mutex<QueueReaderInner<Msg>>,
}

struct QueueReaderInner<Msg: Message> {
    rx: Receiver<Enqueued<Msg>>,
    next_item: Option<Enqueued<Msg>>
}

impl<Msg: Message> QueueReader<Msg> {
    pub fn dequeue(&self) -> Enqueued<Msg> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(item) = inner.next_item.take() {
            item
        } else {
            inner.rx.recv().unwrap()
        }
        
    }

    pub fn try_dequeue(&self) -> DequeueResult<Enqueued<Msg>> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(item) = inner.next_item.take() {
            Ok(item)
        } else {
            inner.rx.try_recv().map_err(|_| QueueEmpty)
        }
    }
    
    pub fn has_msgs(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.next_item.is_some() || {
            match inner.rx.try_recv() {
                Ok(item) => {
                    inner.next_item = Some(item);
                    true
                },
                Err(_) => false
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnqueueError<T> {
    pub msg: T
}

pub type EnqueueResult<Msg> = Result<(), EnqueueError<Enqueued<Msg>>>;

pub struct QueueEmpty;
pub type DequeueResult<Msg> = Result<Msg, QueueEmpty>;
