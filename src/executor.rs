use config::Config;
use futures::{
    channel::oneshot::Receiver,
    task::{Context as PollContext, Poll},
    Future,
};
use std::{error::Error, pin::Pin, sync::Arc};

pub type ExecutorHandle = Arc<dyn TaskExecutor>;

pub trait Task: Future<Output = ()> + Send {}
impl<T: Future<Output = ()> + Send> Task for T {}

pub trait TaskExecutor {
    fn spawn(&self, future: Pin<Box<dyn Task>>) -> Result<Box<dyn TaskExec<()>>, Box<dyn Error>>;
}
pub trait TaskExec<T: Send>:
    Future<Output = Result<T, Box<dyn Error>>> + Unpin + Send + Sync
{
    fn abort(self: Box<Self>);
    fn forget(self: Box<Self>);
}
pub struct TaskHandle<T: Send> {
    handle: Box<dyn TaskExec<()>>,
    recv: Receiver<T>,
}
impl<T: Send> TaskHandle<T> {
    pub fn new(handle: Box<dyn TaskExec<()>>, recv: Receiver<T>) -> Self {
        Self { handle, recv }
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
impl<T: Send> TaskHandle<T> {
    pub fn abort(self) {
        self.handle.abort()
    }
    pub fn forget(self) {
        self.handle.forget()
    }
}
impl<T: Send> TaskExec<T> for TaskHandle<T> {
    fn abort(self: Box<Self>) {
        self.handle.abort()
    }
    fn forget(self: Box<Self>) {
        self.handle.forget()
    }
}

pub use executor_impl::*;
#[cfg(feature = "tokio_executor")]
mod executor_impl {
    pub fn get_executor_handle(_: &Config) -> ExecutorHandle {
        Arc::new(TokioExecutor(tokio::runtime::Handle::current()))
    }
    use super::*;
    pub struct TokioExecutor(pub tokio::runtime::Handle);
    impl TaskExecutor for TokioExecutor {
        fn spawn(
            &self,
            future: Pin<Box<dyn Task>>,
        ) -> Result<Box<dyn TaskExec<()>>, Box<dyn Error>> {
            Ok(Box::new(TokioJoinHandle(self.0.spawn(future))))
        }
    }
    struct TokioJoinHandle(tokio::task::JoinHandle<()>);
    impl Future for TokioJoinHandle {
        type Output = Result<(), Box<dyn Error>>;
        fn poll(mut self: Pin<&mut Self>, cx: &mut PollContext<'_>) -> Poll<Self::Output> {
            Future::poll(Pin::new(&mut self.0), cx)
                .map_err(|e| Box::new(e) as Box<dyn Error + 'static>)
        }
    }
    impl TaskExec<()> for TokioJoinHandle {
        fn abort(self: Box<Self>) {
            self.0.abort();
        }
        fn forget(self: Box<Self>) {
            drop(self);
        }
    }
}

#[cfg(not(feature = "tokio_executor"))]
mod executor_impl {
    use super::*;
    use crate::system::ThreadPoolConfig;
    use futures::task::SpawnExt;
    pub fn get_executor_handle(cfg: &Config) -> ExecutorHandle {
        let exec_cfg = ThreadPoolConfig::from(cfg);
        let pool = futures::executor::ThreadPoolBuilder::new()
            .pool_size(exec_cfg.pool_size)
            .stack_size(exec_cfg.stack_size)
            .name_prefix("pool-thread-#")
            .create()
            .unwrap();
        Arc::new(FuturesExecutor(pool))
    }
    pub struct FuturesExecutor(pub futures::executor::ThreadPool);
    impl TaskExecutor for FuturesExecutor {
        fn spawn(
            &self,
            future: Pin<Box<dyn Task>>,
        ) -> Result<Box<dyn TaskExec<()>>, Box<dyn Error>> {
            self.0
                .spawn_with_handle(future)
                .map(|h| Box::new(FuturesJoinHandle(h)) as Box<dyn TaskExec<()>>)
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
    impl TaskExec<()> for FuturesJoinHandle {
        fn abort(self: Box<Self>) {
            drop(self)
        }
        fn forget(self: Box<Self>) {
            self.0.forget()
        }
    }
}
