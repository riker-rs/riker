use std::collections::{HashMap, HashSet};
use std::thread;
use std::panic::catch_unwind;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::ops::Deref;
use std::pin::{Pin, Unpin};

pub use futures::future::*;
use futures::{pending, poll, FutureExt, TryFutureExt};
use futures::executor::block_on;
use config::Config;
use log::{log, trace, warn};
use pin_utils::pin_mut;

use crate::protocol::{Message, SystemMsg, SystemEvent, ChannelMsg};
use crate::actor::{BoxActor, ActorCell, CellInternal};
use crate::actor::{ActorRef, ActorId, ActorUri, BoxActorProd};
use crate::actor::{TryTell, SysTell, ActorProducer, CreateError, RestartError};
use crate::kernel::{KernelRef, KernelMsg, Dispatcher, BigBang, create_actor_ref};
use crate::kernel::{Mailbox, MailboxSender, MailboxConfig, mailbox, run_mailbox, flush_to_deadletters};
use crate::system::{ActorSystem, Job};

use self::KernelMsg::{CreateActor, RestartActor, TerminateActor};
use self::KernelMsg::{Initialize, ParkActor, UnparkActor, Stop, RunFuture};

pub struct Kernel<Msg: Message, Dis> {
    dispatcher: Dis,
    actors: HashMap<ActorId, ActorDock<Msg>>,
    uris: HashSet<ActorUri>,
    mailbox_conf: MailboxConfig,
}

impl<Msg, Dis> Kernel<Msg, Dis>
    where Msg: Message, Dis: Dispatcher {

    pub fn new(config: &Config) -> Kernel<Msg, Dis> {
        // let debug = config.get_bool("debug").unwrap();
        let dispatcher = Dis::new(config, false); //todo decide if we're using debug
        let actors = HashMap::new();
        let uris = HashSet::new();
        let mailbox_conf = MailboxConfig::from(config);

        Kernel {
            dispatcher,
            actors,
            uris,
            mailbox_conf
        }
    }

    pub fn start(mut self,
                system: &ActorSystem<Msg>,
                timer_tx: Sender<Job<Msg>>) -> (KernelRef<Msg>, SysActors<Msg>) {
        let (kernel_tx, kernel_rx) = channel::<KernelMsg<Msg>>();

        let kernel_ref = KernelRef {
            kernel_tx,
            timer_tx
        };

        let mut system = system.clone();
        system.kernel = Some(kernel_ref.clone());

        let mut bb = BigBang::new(&kernel_ref, &system);
        self.actors.insert(bb.user.0.uri.uid, bb.user.1.take().unwrap());
        self.actors.insert(bb.sysm.0.uri.uid, bb.sysm.1.take().unwrap());
        self.actors.insert(bb.temp.0.uri.uid, bb.temp.1.take().unwrap());

        let sys_actors = SysActors {
            root: bb.root.clone(),
            user: bb.user.0.clone(),
            sysm: bb.sysm.0.clone(),
            temp: bb.temp.0.clone()
        };

        self.spawn(system.clone(), kernel_rx, kernel_ref.clone());

        (kernel_ref, sys_actors)
    }

    fn spawn(self,
            mut system: ActorSystem<Msg>,
            kernel_rx: Receiver<KernelMsg<Msg>>,
            kernel_ref: KernelRef<Msg>) {

        let mut kernel = self;

        thread::spawn(move || {
            
            let mut uid_counter = 10;
            let mut events: Option<ActorRef<Msg>> = None;
            let mut dead: Option<ActorRef<Msg>> = None;

            loop {
                let message = kernel_rx.recv().unwrap();
                match message {
                    Initialize(sys) => {
                        system = sys;
                        events = Some(system.event_stream().clone());
                        dead = Some(system.dead_letters().clone());
                    }
                    CreateActor(props, name, parent, tx) => {
                        let result = kernel.create_actor(
                                            uid_counter,
                                            &system,
                                            props,
                                            name,
                                            parent,
                                            &kernel_ref);

                        if let Ok(ref actor) = result {
                            uid_counter += 1;
                            if events.is_some() {
                                publish_event(&events, SystemEvent::ActorCreated(actor.clone()));
                            }
                        }

                        drop(tx.send(result));
                    }
                    TerminateActor(uid) => {
                        if let Ok(terminated) = kernel.terminate_actor(uid, dead.as_ref().unwrap()) {
                            publish_event(&events, SystemEvent::ActorTerminated(terminated));
                        }
                    }
                    RestartActor(uid) => {
                        if let Ok(restarted) = kernel.restart_actor(uid) {
                            publish_event(&events, SystemEvent::ActorRestarted(restarted));
                        }
                    }
                    ParkActor(uid, actor) => kernel.park_actor(uid, actor),
                    UnparkActor(uid) => kernel.unpark_actor(uid),
                    RunFuture(f) => {
                        println!("RF");
                        // pin_mut!(f);
                        kernel.dispatcher.execute(f);
                    },

                    // break out of the main loop and let self be consumed
                    // thus removing the entire kernel from memory
                    // break;
                    Stop => break
                };
            }
        });
    }

    fn create_actor(&mut self,
                    uid: ActorId,
                    system: &ActorSystem<Msg>,
                    props: BoxActorProd<Msg>,
                    name: String,
                    parent: ActorRef<Msg>,
                    kernel_ref: &KernelRef<Msg>)
                    -> Result<ActorRef<Msg>, CreateError> {
        let uri = ActorUri {
            uid,
            path: Arc::new(format!("{}/{}", parent.uri.path, name)),
            name: Arc::new(name),
            host: system.proto.host.clone()
        };

        trace!("Attempting to create actor at: {}", uri.path);

        if self.uris.contains(&uri) {
           return Err(CreateError::AlreadyExists(uri.path.deref().clone()));
        }
        
        let (sender, mailbox) = mailbox::<Msg>(uri.uid,
                                                self.mailbox_conf.msg_process_limit,
                                                kernel_ref.clone());
        let actor = start_actor(&props)?;

        let actor_ref = create_actor_ref(&parent,
                                        &uri,
                                        kernel_ref,
                                        system,
                                        Some(sender.clone()),
                                        actor.persistence_conf()); // todo remove .clone?

        parent.cell.add_child(uri.name.as_str(), actor_ref.clone());
        trace!("Actor created: {}", actor_ref);
        
        let dock = ActorDock {
            actor: Some(actor),
            cell: actor_ref.cell.clone(),
            actor_ref: actor_ref.clone(),
            mailbox_sender: sender,
            mailbox: mailbox,
            props: props
        }; 
        
        self.actors.insert(uri.uid, dock);
        self.uris.insert(uri);
        actor_ref.sys_tell(SystemMsg::ActorInit, None);
        Ok(actor_ref)
    }

    fn restart_actor(&mut self, uid: ActorId) -> Result<ActorRef<Msg>, RestartError> {
        match self.actors.get_mut(&uid) {
            Some(dock) => {
                let actor = start_actor(&dock.props)
                                .map_err(|_| RestartError)?;

                dock.actor = Some(actor);
                let restarted = dock.actor_ref.clone();
                restarted.sys_tell(SystemMsg::ActorInit, None);
                Ok(restarted)
            }
            _ => {
                warn!("Cannot restart an already terminated actor {}", uid);
                Err(RestartError)
            }
        }
    }

    fn terminate_actor(&mut self, uid: ActorId, dl: &ActorRef<Msg>) -> Result<ActorRef<Msg>, ()> {
        match self.actors.remove(&uid) {
            Some(dock) => {
                let terminated = dock.actor_ref.clone();

                flush_to_deadletters(&dock.mailbox, dl, &terminated.uri);
                self.uris.remove(&terminated.uri);

                let parent = terminated.parent();

                if !parent.is_root() {
                    let term_msg = SystemMsg::Event(SystemEvent::ActorTerminated(terminated.clone()));
                    parent.sys_tell(term_msg, None);
                }
                Ok(terminated)
            }
            _ => {
                warn!("Cannot terminate an already terminated actor {}", uid);
                Err(())
            }
        }
    }

    fn park_actor(&mut self, uid: ActorId, actor: Option<BoxActor<Msg>>) {
        // if the actor dock does not exist in the hashmap it means that
        // the actor has been terminated during mailbox execution
        if let Some(dock) = self.actors.get_mut(&uid) {
            if actor.is_some() {
                dock.actor = actor;
            }
        }
    }

    fn unpark_actor(&mut self, uid: ActorId) {
        match self.actors.get_mut(&uid) {
            Some(dock) => {
                let cell = dock.cell.clone();
                let mbox = dock.mailbox.clone();
                let actor = dock.actor.take();

                let f = async {
                    run_mailbox(mbox, cell, actor);
                };

                let run = async {
                    await!(AssertUnwindSafe(f).catch_unwind());
                    ()
                };

                let run = self.dispatcher.execute(run);

                // self.dispatcher.execute(async {
                //     pin_mut!(run);
                // });
            }
            _ => {}
        }
    }
}

fn start_actor<Msg>(props: &BoxActorProd<Msg>) -> Result<BoxActor<Msg>, CreateError>
    where Msg: Message
{
    let actor = catch_unwind(|| props.produce()).map_err(|_| {
            CreateError::Panicked
    })?;

    Ok(actor)
}

pub struct ActorDock<Msg: Message> {
    pub actor: Option<BoxActor<Msg>>,
    pub cell: ActorCell<Msg>,
    pub actor_ref: ActorRef<Msg>,
    pub mailbox_sender: MailboxSender<Msg>,
    pub mailbox: Mailbox<Msg>,
    pub props: BoxActorProd<Msg>,
}

#[derive(Clone)]
pub struct SysActors<Msg: Message> {
    pub root: ActorRef<Msg>,
    pub user: ActorRef<Msg>,
    pub sysm: ActorRef<Msg>,
    pub temp: ActorRef<Msg>,
}

unsafe impl<Msg: Message, Dis: Dispatcher> Send for Kernel<Msg, Dis> {}
unsafe impl<Msg: Message, Dis: Dispatcher> Sync for Kernel<Msg, Dis> {}

fn publish_event<Msg>(es: &Option<ActorRef<Msg>>, evt: SystemEvent<Msg>)
    where Msg: Message
{
    let _ = es.try_tell(ChannelMsg::PublishEvent(evt), None);
}
