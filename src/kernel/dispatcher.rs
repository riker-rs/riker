use config::Config;
use futures::Future;

pub trait Dispatcher : 'static {
    fn new(config: &Config, debug: bool) -> Self;

    fn execute<F>(&mut self, f: F)
        where F: Future<Output = ()> + Send + 'static;
}
