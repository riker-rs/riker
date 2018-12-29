#![feature(futures_api)]

use futures::Future;
use futures::executor::{ThreadPool, ThreadPoolBuilder};
use futures::task::{SpawnExt};
use futures::future::{FutureObj, UnsafeFutureObj, RemoteHandle};
use config::Config;


use riker::kernel::{Dispatcher, RikerFuture};

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

    // fn execute<F>(&mut self, f: F)
    //     where F: Future<Output=()> + Send + 'static
    // {
    //     //self.inner.run(spawn(f)).unwrap();
    //     self.inner.run(f.f)
    // }
    // fn execute<F: Future + Send + 'static>(&mut self, f: F) -> RemoteHandle<F::Output> {
    //     // self.inner.run(f)
    //     self.inner.spawn_with_handle(f).unwrap()
    // }
    fn execute<F>(&mut self, f: F) -> RemoteHandle<F::Output>
        where F: Future + Send + 'static,
                <F as Future>::Output: std::marker::Send
    {
        self.inner.spawn_with_handle(f).unwrap()
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
