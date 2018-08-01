extern crate config;
extern crate futures;
extern crate riker;

use futures::{Future, Never};
use futures::executor::{ThreadPool, ThreadPoolBuilder};
use config::Config;

use riker::kernel::Dispatcher;
use riker::futures_util::spawn;

pub struct ThreadPoolDispatcher {
    inner: ThreadPool,
}

impl Dispatcher for ThreadPoolDispatcher {
    fn new(config: &Config, _: bool) -> ThreadPoolDispatcher {
        let config = ThreadPoolConfig::from(config);
        ThreadPoolDispatcher {
            inner: ThreadPoolBuilder::new()
                                        .pool_size(config.pool_size)
                                        .name_prefix("pool-thread-#")
                                        .create()
                                        .unwrap()
        }
    }

    fn execute<F>(&mut self, f: F)
        where F: Future<Item=(), Error=Never> + Send + 'static
    {
        self.inner.run(spawn(f)).unwrap();
    }
}

struct ThreadPoolConfig {
    pool_size: usize,
}

impl<'a> From<&'a Config> for ThreadPoolConfig {
    fn from(config: &Config) -> Self {
        ThreadPoolConfig {
            pool_size: config.get_int("dispatcher.pool_size").unwrap() as usize
        }
    }
}
