use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

use futures::{channel::mpsc::channel, task::SpawnExt, StreamExt};
use log::warn;

use crate::{
    actor::actor_cell::ExtendedCell,
    actor::*,
    kernel::{
        kernel_ref::KernelRef,
        mailbox::{flush_to_deadletters, run_mailbox, Mailbox},
        KernelMsg,
    },
    system::{ActorRestarted, ActorSystem, ActorTerminated, SystemMsg},
    Message,
};

pub struct Dock<A: Actor> {
    pub actor: Arc<Mutex<Option<A>>>,
    pub cell: ExtendedCell<A::Msg>,
}

impl<A: Actor> Clone for Dock<A> {
    fn clone(&self) -> Dock<A> {
        Dock {
            actor: self.actor.clone(),
            cell: self.cell.clone(),
        }
    }
}

pub fn kernel<A>(
    props: BoxActorProd<A>,
    cell: ExtendedCell<A::Msg>,
    mailbox: Mailbox<A::Msg>,
    sys: &ActorSystem,
) -> Result<KernelRef, CreateError>
where
    A: Actor + 'static,
{
    let (tx, mut rx) = channel::<KernelMsg>(1000); // todo config?
    let kr = KernelRef { tx };

    let mut sys = sys.clone();
    let mut asys = sys.clone();
    let akr = kr.clone();
    let actor = start_actor(&props)?;
    let cell = cell.init(&kr);

    let dock = Dock {
        actor: Arc::new(Mutex::new(Some(actor))),
        cell: cell.clone(),
    };

    let actor_ref = ActorRef::new(cell);

    let f = async move {
        while let Some(msg) = rx.next().await {
            match msg {
                KernelMsg::RunActor => {
                    let ctx = Context {
                        myself: actor_ref.clone(),
                        system: asys.clone(),
                        kernel: akr.clone(),
                    };

                    let mb = mailbox.clone();
                    let d = dock.clone();

                    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| run_mailbox(mb, ctx, d))); //.unwrap();
                }
                KernelMsg::RestartActor => {
                    restart_actor(&dock, actor_ref.clone().into(), &props, &asys);
                }
                KernelMsg::TerminateActor => {
                    terminate_actor(&mailbox, actor_ref.clone().into(), &asys);
                    break;
                }
                KernelMsg::Sys(s) => {
                    asys = s;
                }
            }
        }
    };

    sys.exec.spawn(f).unwrap();
    Ok(kr)
}

fn restart_actor<A>(
    dock: &Dock<A>,
    actor_ref: BasicActorRef,
    props: &BoxActorProd<A>,
    sys: &ActorSystem,
) where
    A: Actor,
{
    let mut a = dock.actor.lock().unwrap();
    match start_actor(props) {
        Ok(actor) => {
            *a = Some(actor);
            actor_ref.sys_tell(SystemMsg::ActorInit);
            sys.publish_event(ActorRestarted { actor: actor_ref }.into());
        }
        Err(_) => {
            warn!("Actor failed to restart: {:?}", actor_ref);
        }
    }
}

fn terminate_actor<Msg>(mbox: &Mailbox<Msg>, actor_ref: BasicActorRef, sys: &ActorSystem)
where
    Msg: Message,
{
    sys.provider.unregister(actor_ref.path());
    flush_to_deadletters(mbox, &actor_ref, sys);
    sys.publish_event(
        ActorTerminated {
            actor: actor_ref.clone(),
        }
        .into(),
    );

    let parent = actor_ref.parent();
    if !parent.is_root() {
        parent.sys_tell(ActorTerminated { actor: actor_ref }.into());
    }
}

fn start_actor<A>(props: &BoxActorProd<A>) -> Result<A, CreateError>
where
    A: Actor,
{
    let actor = catch_unwind(|| props.produce()).map_err(|_| CreateError::Panicked)?;

    Ok(actor)
}
