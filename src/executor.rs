use futures::{
    task::{
        Poll,
        Context as PollContext,
        SpawnExt,
    },
    channel::oneshot::Receiver,
    Future,
};
use std::{
    error::Error,
    pin::{
        Pin,
    },
    sync::Arc,
};

pub type ExecutorHandle = Arc<dyn TaskExecutor>;
pub fn get_executor_handle() -> ExecutorHandle {
    Arc::new(TokioExecutor(tokio::runtime::Handle::current()))
}

pub trait Task: Future<Output=()> + Send { }
impl<T: Future<Output=()> + Send> Task for T { }

pub trait TaskExecutor {
    fn spawn(&self, future: Pin<Box<dyn Task>>) -> Result<Box<dyn TaskExec>, Box<dyn Error>>;
}
pub trait TaskExec: Future<Output=Result<(), Box<dyn Error>>> + Unpin + Send + Sync {
    fn abort(self);
    fn forget(self);
}
pub struct TaskHandle<T: Send> {
    handle: Box<dyn TaskExec>,
    recv: Receiver<T>,
}
impl<T: Send> TaskHandle<T> {
    pub fn new(handle: Box<dyn TaskExec>, recv: Receiver<T>) -> Self {
        Self {
            handle,
            recv,
        }
    }
}
impl<T: Send> Future for TaskHandle<T> {
    type Output = Result<T, Box<dyn Error>>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut PollContext<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(_) = TaskExec::poll(Pin::new(&mut *self.handle), cx) {
            if let Poll::Ready(val) = <Receiver<T> as Future>::poll(Pin::new(&mut self.recv), cx) {
                self.recv.close();
                return Poll::Ready(val.map_err(|e| Box::new(e) as Box<dyn Error + 'static>));
            }
        }
        Poll::Pending
    }
}

struct TokioExecutor(tokio::runtime::Handle);
impl TaskExecutor for TokioExecutor {
    fn spawn(&self, future: Pin<Box<dyn Task>>) -> Result<Box<dyn TaskExec>, Box<dyn Error>> {
        Ok(Box::new(TokioJoinHandle(self.0.spawn(future))))
    }
}
struct TokioJoinHandle(tokio::task::JoinHandle<()>);
impl Future for TokioJoinHandle {
    type Output = Result<(), Box<dyn Error>>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut PollContext<'_>) -> Poll<Self::Output> {
        Future::poll(Pin::new(&mut self.0), cx).map_err(|e| Box::new(e) as Box<dyn Error + 'static>)
    }
}
impl TaskExec for TokioJoinHandle {
    fn abort(self) {
        self.0.abort();
    }
    fn forget(self) {
        drop(self);
    }
}

struct FuturesExecutor(futures::executor::ThreadPool);
impl TaskExecutor for FuturesExecutor {
    fn spawn(&self, future: Pin<Box<dyn Task>>) -> Result<Box<dyn TaskExec>, Box<dyn Error>> {
        self.0.spawn_with_handle(future)
            .map(|h| Box::new(FuturesJoinHandle(h)) as Box<dyn TaskExec>)
            .map_err(|e| Box::new(e) as Box<dyn Error>)
    }
}
struct FuturesJoinHandle(futures::future::RemoteHandle<()>);
impl Future for FuturesJoinHandle {
    type Output = Result<(), Box<dyn Error>>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut PollContext<'_>) -> Poll<Self::Output> {
        Future::poll(Pin::new(&mut self.0), cx).map(|_| Ok(()) as Result<(), Box<dyn Error>>)
    }
}
impl TaskExec for FuturesJoinHandle {
    fn abort(self) {
        drop(self.0)
    }
    fn forget(self) {
        self.0.forget();
    }
}
