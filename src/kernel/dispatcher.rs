use config::Config;
use std::pin::{Pin, Unpin};

use pin_utils::{unsafe_pinned, unsafe_unpinned};

use futures::task::{LocalWaker};
use futures::future::RemoteHandle;
pub use futures::{Future, Poll, task};
pub use futures::channel::oneshot::{channel as dispatch, Receiver};


pub struct RikerFuture {
    pub f: Box<dyn Future<Output=()> + Send + 'static>,
}

// impl RikerFuture {
//         // unsafe_pinned!(f: Box<dyn Future<Output=()> + Send + 'static>);
//         // unsafe_unpinned!(f: Box<dyn Future<Output=()> + Send + 'static>);
// }

impl Future for RikerFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<()> {
        let p = self.poll(lw);
        p
    }

}

impl Unpin for RikerFuture {}

pub trait Dispatcher : 'static {
    fn new(config: &Config, debug: bool) -> Self;

    // fn execute<F>(&mut self, f: F)
    //     where F: Future<Output=()> + Send + 'static;
    //     fn execute(&mut self, f: impl Future<Output=()> + Send + 'static);

    fn execute<F>(&mut self, f: F) -> RemoteHandle<F::Output>
        where F: Future + Send + 'static,
                <F as Future>::Output: std::marker::Send;

        // fn execute(&mut self, f: RikerFuture);
}
