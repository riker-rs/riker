use futures::{Future, Never, Async, Poll, FutureExt};
use std::fmt::Debug;
use std::panic::AssertUnwindSafe;

use futures::task::Context;
use futures::channel::oneshot::{channel, Sender, Receiver};

pub struct DispatchHandle<T, E> {
    pub inner: Receiver<Result<T, E>>,
}

impl<T, E> Future for DispatchHandle<T, E>
    where T: Send + 'static,
            E: Send + Debug + 'static
{
    type Item = T;
    type Error = E;

    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Item, Self::Error> {
        match self.inner.poll(cx).expect("") {
            Async::Ready(Ok(e)) => Ok(e.into()),
            Async::Ready(Err(e)) => Err(e),
            Async::Pending => Ok(Async::Pending),
        }
    }
}

pub struct MySender<F, T> {
    pub fut: F,
    pub tx: Option<Sender<T>>,
}

impl<F: Future> Future for MySender<F, Result<F::Item, F::Error>> {
    type Item = ();
    type Error = Never;

    fn poll(&mut self, cx: &mut Context) -> Poll<(), Never> {
        let res = match self.fut.poll(cx) {
            Ok(Async::Ready(e)) => Ok(e),
            Ok(Async::Pending) => return Ok(Async::Pending),
            Err(e) => Err(e),
        };

        // if the receiving end has gone away then that's ok, we just ignore the
        // send error here.
        drop(self.tx.take().unwrap().send(res));
        Ok(Async::Ready(()))
    }
}


/// A future representing the completion of task spawning.
///
/// See [`spawn`](spawn()) for details.
#[derive(Debug)]
pub struct Spawn<F>(Option<F>);

/// Spawn a task onto the default executor.
///
/// This function returns a future that will spawn the given future as a task
/// onto the default executor. It does *not* provide any way to wait on task
/// completion or extract a value from the task. That can either be done through
/// a channel, or by using [`spawn_with_handle`](::spawn_with_handle).
pub fn spawn<F>(f: F) -> Spawn<F>
    where F: Future<Item = (), Error = Never> + 'static + Send
{
    Spawn(Some(f))
}

impl<F: Future<Item = (), Error = Never> + Send + 'static> Future for Spawn<F> {
    type Item = ();
    type Error = Never;
    fn poll(&mut self, cx: &mut Context) -> Poll<(), Never> {
        let (tx, _rx) = channel(); // todo remove. tx/rx not used. tx only used to satisfy MySender

        let sender = MySender {
            fut: AssertUnwindSafe(self.0.take().unwrap()).catch_unwind(),
            tx: Some(tx),
        };

        cx.spawn(sender);
        Ok(Async::Ready(()))
    }
}

