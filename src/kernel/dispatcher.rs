use config::Config;

pub use futures::{Future, Poll, Async, Never, task};
pub use futures::channel::oneshot::{channel as dispatch, Receiver};
pub use futures::executor::JoinHandle;

pub trait Dispatcher : 'static {
    fn new(config: &Config, debug: bool) -> Self;

    fn execute<F>(&mut self, f: F)
        where F: Future<Item=(), Error=Never> + Send + 'static;
}
