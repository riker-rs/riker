use std::sync::{Arc, Mutex};
use std::marker::PhantomData;
use std::pin::Pin;

use chrono::prelude::{DateTime, Utc};
use config::Config;
use log::warn;

use futures::{Future, FutureExt};
use futures::channel::oneshot::{channel, Sender, Canceled};

use crate::protocol::{Message, ActorMsg, ESMsg, SystemMsg};
use crate::actors::{Actor, BoxActor, Context, ActorRef, BoxActorProd};
use crate::actors::{Props, ActorRefFactory, TmpActorRefFactory, Tell, SysTell};

// use actor::BoxActorProd;

// pub trait EsManagerProps {
//     type Msg: Message;
//     type Evs: EventStore;

//     fn props(config: &Config) -> Option<BoxActorProd<Self::Msg>>;
// }

// pub fn es_manager<Evs>(config: &Config) -> BoxActor<Evs::Msg>
//     where Evs: EventStore
// {
    

// }

pub struct EsManager<Evs: EventStore> {
    es: Evs,
}

impl<Evs: EventStore> EsManager<Evs> {
    fn new(es: Evs) -> BoxActor<Evs::Msg> {
        let actor: EsManager<Evs> = EsManager {
            es: es,
        };

        Box::new(actor)
    }

    pub fn props(config: &Config) -> BoxActorProd<Evs::Msg> {
        let es = Evs::new(config);
        Props::new_args(Box::new(EsManager::new), es)
    }
}

impl<Evs: EventStore> Actor for EsManager<Evs> {
    type Msg = Evs::Msg;

    fn other_receive(&mut self,
                    _: &Context<Self::Msg>,
                    msg: ActorMsg<Self::Msg>,
                    sender: Option<ActorRef<Self::Msg>>) {

        if let ActorMsg::ES(msg) = msg {
            match msg {
                ESMsg::Persist(evt, id, keyspace) => {
                    self.es.insert(&id, &keyspace, evt.clone());

                    sender.unwrap().sys_tell(SystemMsg::Persisted(evt.msg), None);
                }
                ESMsg::Load(id, keyspace) => {
                    let result = self.es.load(&id, &keyspace);
                    sender.unwrap().tell(ESMsg::LoadResult(result), None);
                }
                _ => {}
            }
        }
    }

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
}

pub trait EventStore : Clone + Send + Sync + 'static {
    type Msg: Message;

    fn new(config: &Config) -> Self;

    fn insert(&mut self, id: &String, keyspace: &String, evt: Evt<Self::Msg>);

    fn load(&self, id: &String, keyspace: &String) -> Vec<Self::Msg>;
}

#[derive(Clone, Debug)]
pub struct Evt<Msg: Message> {
    pub date: DateTime<Utc>,
    pub msg: Msg,
}

impl<Msg: Message> Evt<Msg> {
    pub fn new(msg: Msg) -> Self {
        Evt {
            date: Utc::now(),
            msg: msg
        }
    }
}

// #[allow(dead_code)]
// pub struct NoPersist<Evs: EventStore> {
//     es: Evs,
// }

// #[allow(dead_code)]
// impl<Evs: EventStore> NoPersist<Evs> {
//     fn new(es: Evs) -> BoxActor<Evs::Msg>
//     {
//         let actor: NoPersist<Evs> = NoPersist {
//             es: es,
//         };

//         Box::new(actor)
//     }
// }

// impl<Evs: EventStore> Actor for NoPersist<Evs> {
//     type Msg = Evs::Msg;

//     fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}
// }

// impl<Evs: EventStore> EsManagerProps for NoPersist<Evs> {
//     type Msg = Evs::Msg;
//     type Evs = Evs;

//     fn props(_config: &Config) -> Option<BoxActorProd<Self::Msg>> {
//         None
//     }
// }


#[derive(Clone)]
pub struct NoEventStore<Msg: Message> {
    msg: Arc<Mutex<PhantomData<Msg>>>,
}

impl<Msg: Message> EventStore for NoEventStore<Msg> {
    type Msg = Msg;

    fn new(_config: &Config) -> Self {
        NoEventStore {
            msg: Arc::new(Mutex::new(PhantomData))
        }
    }

    fn insert(&mut self, _: &String, _: &String, _: Evt<Msg>) {
        warn!("No event store configured");
        
    }

    fn load(&self, _: &String, _: &String) -> Vec<Msg> {
        warn!("No event store configured");
        vec![]
    }
}

struct EsQueryActor<Msg: Message> {
    tx: Arc<Mutex<Option<Sender<Vec<Msg>>>>>,
}

impl<Msg: Message> EsQueryActor<Msg> {
    fn new(tx: Arc<Mutex<Option<Sender<Vec<Msg>>>>>) -> BoxActor<Msg> {
        let ask = EsQueryActor {
            tx: tx
        };
        Box::new(ask)
    }

    fn fulfill_query(&self, events: Vec<Msg>) {
        match self.tx.lock() {
            Ok(mut tx) => drop(tx.take().unwrap().send(events)),
            _ => {}
        }
    }
}

impl<Msg: Message> Actor for EsQueryActor<Msg> {
    type Msg = Msg;

    fn other_receive(&mut self, ctx: &Context<Msg>, msg: ActorMsg<Msg>, _: Option<ActorRef<Msg>>) {
        match msg {
            ActorMsg::ES(result) => {
                if let ESMsg::LoadResult(events) = result {
                    self.fulfill_query(events);
                    ctx.stop(&ctx.myself);
                }
            }
            _ => {}
        }
    }

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {
        
    }
}

type QueryFuture<Msg> = Pin<Box<dyn Future<Output=Result<Vec<Msg>, Canceled>> + Send>>;

pub fn query<Msg, Ctx>(id: &String,
                        keyspace: &String,
                        es: &ActorRef<Msg>,
                        ctx: &Ctx) -> QueryFuture<Msg>
    where Msg: Message, Ctx: TmpActorRefFactory<Msg=Msg>
{
    let (tx, rx) = channel::<Vec<Msg>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let props = Props::new_args(Box::new(EsQueryActor::new), tx);
    let actor = ctx.tmp_actor_of(props).unwrap();
    es.tell(ESMsg::Load(id.clone(), keyspace.clone()), Some(actor));

    // Box::new(rx)
    rx.boxed()
}
