use futures::{Future, FutureExt};
use std::fmt::Debug;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;

use futures::task::{LocalWaker, Poll};
use futures::channel::oneshot::{channel, Sender, Receiver};

// pub struct DispatchHandle<T> {
//     pub inner: Receiver<T>,
// }

// impl<T> Future for DispatchHandle<T>
//     where T: Send + 'static,
// {
//     type Output = T;

//     fn poll(self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<Self::Output> {
//         // TODO r2018
//         // match self.inner.poll(cx).expect("") {
//         //     Poll::Ready(Ok(e)) => e.into(),
//         //     Poll::Ready(Err(e)) => e,
//         //     Poll::Pending => Poll::Pending,
//         // }
//         Poll::Pending
//     }
// }

pub struct MySender<F, T> {
    pub fut: F,
    pub tx: Option<Sender<T>>,
}

impl<F: Future> Future for MySender<F, F::Output> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<Self::Output> {
        // TODO r2018
        // let res = match self.fut.poll(cx) {
        //     Poll::Ready(e) => e,
        //     Poll::Pending => return Poll::Pending,
        //     // Err(e) => Err(e),
        // };

        // // if the receiving end has gone away then that's ok, we just ignore the
        // // send error here.
        // drop(self.tx.take().unwrap().send(res));
        Poll::Ready(())
    }
}


// /// A future representing the completion of task spawning.
// ///
// /// See [`spawn`](spawn()) for details.
// #[derive(Debug)]
// pub struct Spawn<F>(Option<F>);

// /// Spawn a task onto the default executor.
// ///
// /// This function returns a future that will spawn the given future as a task
// /// onto the default executor. It does *not* provide any way to wait on task
// /// completion or extract a value from the task. That can either be done through
// /// a channel, or by using [`spawn_with_handle`](::spawn_with_handle).
// pub fn spawn<F>(f: F) -> Spawn<F>
//     where F: Future<Output = ()> + 'static + Send
// {
//     Spawn(Some(f))
// }

// impl<F: Future<Output = ()> + Send + 'static> Future for Spawn<F> {
//     type Output = ();

//     fn poll(self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<Self::Output> {
//         // TODO r2018
//         // let (tx, _rx) = channel(); // todo remove. tx/rx not used. tx only used to satisfy MySender

//         // let sender = MySender {
//         //     fut: AssertUnwindSafe(self.0.take().unwrap()).catch_unwind(),
//         //     tx: Some(tx),
//         // };

//         // cx.spawn(sender);
//         Poll::Ready(())
//     }
// }

