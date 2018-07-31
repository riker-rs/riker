extern crate config;
extern crate riker;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use config::Config;

use riker::actors::{Message, Evt};
use riker::system::EventStore;

#[derive(Clone)]
pub struct MapVec<Msg: Message> {
    inner: Arc<Mutex<HashMap<String, Vec<Evt<Msg>>>>>,
}

impl<Msg: Message> EventStore for MapVec<Msg> {
    type Msg = Msg;

    fn new(_config: &Config) -> Self {
        MapVec {
            inner: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    fn insert(&mut self, id: &String, _keyspace: &String, evt: Evt<Msg>) {
        if self.inner.lock().unwrap().contains_key(id) {
            self.inner.lock().unwrap().get_mut(id).unwrap().push(evt);
        } else {
            self.inner.lock().unwrap().insert(id.to_string(), vec![evt]);
        }
    }

    fn load(&self, id: &String, _keyspace: &String) -> Vec<Msg> {
        match self.inner.lock().unwrap().get(id) {
            Some(v) => v.iter().map(|e| e.msg.clone()).collect(),
            None => Vec::new()
        }
    }
}
