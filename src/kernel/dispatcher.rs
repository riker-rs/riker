use std::pin::{Pin, Unpin};

use config::Config;

use futures::task::{LocalWaker};
use futures::future::RemoteHandle;
pub use futures::{Future, Poll, task};
pub use futures::channel::oneshot::{channel as dispatch, Receiver};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

use crate::ExecResult;

pub trait Dispatcher : 'static {
    fn new(config: &Config, debug: bool) -> Self;

    // fn execute<F>(&mut self, f: F)
    //     where F: Future<Output=()> + Send + 'static;
    //     fn execute(&mut self, f: impl Future<Output=()> + Send + 'static);

    fn execute<F>(&mut self, f: F)
        where F: Future<Output = ()> + Send + 'static;
}
