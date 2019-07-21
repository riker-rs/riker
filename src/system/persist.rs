use chrono::prelude::{DateTime, Utc};
use config::Config;
use log::warn;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::actors::{Actor, ActorRef, BoxActor, BoxActorProd, Context};
use crate::actors::{ActorRefFactory, Props, SysTell, Tell, TmpActorRefFactory};
use crate::protocol::{ActorMsg, ESMsg, Message, SystemMsg};

pub struct EsManager<Evs: EventStore> {
    es: Evs,
}

impl<Evs: EventStore> EsManager<Evs> {
    fn new(es: Evs) -> BoxActor<Evs::Msg> {
        let actor: EsManager<Evs> = EsManager {
            es,
        };

        Box::new(actor)
    }

    pub fn props(config: &Config) -> BoxActorProd<Evs::Msg> {
        let es = Evs::new(config);
        Props::new_args(EsManager::new, es)
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
                ESMsg::Persist(evt, id, keyspace, og_sender) => {
                    self.es.insert(&id, &keyspace, evt.clone());
                    sender.unwrap().sys_tell(SystemMsg::Persisted(evt.msg, og_sender), None);
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

pub trait EventStore: Clone + Send + Sync + 'static {
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
            msg,
        }
    }
}

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
    rec: ActorRef<Msg>,
}

impl<Msg: Message> EsQueryActor<Msg> {
    fn actor(rec: ActorRef<Msg>) -> BoxActor<Msg> {
        let actor = EsQueryActor {
            rec
        };
        Box::new(actor)
    }

    fn fulfill_query(&self, evts: Vec<Msg>) {
        self.rec.sys_tell(SystemMsg::Replay(evts), None);
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

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {}
}

// type QueryFuture<Msg> = Pin<Box<dyn Future<Output=Result<Vec<Msg>, Canceled>> + Send>>;

pub fn query<Msg, Ctx>(id: &String,
                       keyspace: &String,
                       es: &ActorRef<Msg>,
                       ctx: &Ctx,
                       rec: ActorRef<Msg>)
    where Msg: Message, Ctx: TmpActorRefFactory<Msg=Msg>
{
    let props = Props::new_args(EsQueryActor::actor, rec);
    let actor = ctx.tmp_actor_of(props).unwrap();
    es.tell(ESMsg::Load(id.clone(), keyspace.clone()), Some(actor));
}
