use std::sync::Arc;
use std::marker::PhantomData;

use log::trace;

use crate::protocol::Message;
use crate::actor::{ActorId, Actor, ActorRef, ActorUri, Context, ActorCell, CellInternal};
use crate::actor::{Props, ActorProducer, BoxActor, BoxActorProd, PersistenceConf};
use crate::kernel::{KernelRef, ActorDock, MailboxSender, mailbox};
use crate::system::ActorSystem;

pub struct BigBang<Msg: Message> {
    pub root: ActorRef<Msg>,
    pub user: (ActorRef<Msg>, Option<ActorDock<Msg>>),
    pub sysm: (ActorRef<Msg>, Option<ActorDock<Msg>>),
    pub temp: (ActorRef<Msg>, Option<ActorDock<Msg>>)
}

impl<Msg: Message> BigBang<Msg> {
    pub fn new(kernel: &KernelRef<Msg>,
                system: &ActorSystem<Msg>) -> Self {
        let root = root(kernel, system);

        let bb = BigBang {
            root: root.clone(),
            user: guardian(1, "user", "/user", &root, kernel, system),
            sysm: guardian(2, "system", "/system", &root, kernel, system),
            temp: guardian(3, "temp", "/temp", &root, kernel, system)
        };

        root.cell.add_child("user", bb.user.0.clone());
        root.cell.add_child("system", bb.sysm.0.clone());
        root.cell.add_child("temp", bb.temp.0.clone());

        bb
    }
}

fn root<Msg: Message>(kernel: &KernelRef<Msg>,
                        system: &ActorSystem<Msg>) -> ActorRef<Msg> {
    let root_uri = ActorUri {
        uid: 0,
        name: Arc::new("root".to_string()),
        path: Arc::new("/".to_string()),
        host: Arc::new("localhost".to_string())
    };

    let bb_cell = ActorCell::new(0, root_uri.clone(), None, kernel, system, None, None);
    let bigbang = ActorRef::new(&root_uri, bb_cell);

    create_actor_ref(&bigbang,
                    &root_uri,
                    kernel,
                    system,
                    None,
                    None)
}

fn guardian<Msg>(uid: ActorId,
                name: &str,
                path: &str,
                root: &ActorRef<Msg>,
                kernel: &KernelRef<Msg>,
                system: &ActorSystem<Msg>)
                -> (ActorRef<Msg>, Option<ActorDock<Msg>>)
    where Msg: Message
{
    let uri = ActorUri {
        uid,
        name: Arc::new(name.to_string()),
        path: Arc::new(path.to_string()),
        host: Arc::new("localhost".to_string())
    };

    let (sender, mailbox) = mailbox::<Msg>(uri.uid, 100, kernel.clone());

    let props: BoxActorProd<Msg> = Props::new_args(Box::new(Guardian::new), name.to_string());
    let actor = props.produce();

    let actor_ref = create_actor_ref(root,
                        &uri,
                        kernel,
                        system,
                        Some(sender.clone()),
                        None);

    let dock = ActorDock {
        actor: Some(actor),
        cell: actor_ref.cell.clone(),
        actor_ref: actor_ref.clone(),
        mailbox_sender: sender,
        mailbox: mailbox,
        props: props
    };
    (actor_ref, Some(dock))
}

pub fn create_actor_ref<Msg>(parent: &ActorRef<Msg>,
                            uri: &ActorUri,
                            kernel: &KernelRef<Msg>,
                            system: &ActorSystem<Msg>,
                            mailbox: Option<MailboxSender<Msg>>,
                            perconf: Option<PersistenceConf>) -> ActorRef<Msg>
    where Msg: Message
{                                           
    let cell = ActorCell::new(uri.uid,
                                uri.clone(),
                                Some(parent.clone()),
                                kernel,
                                system,
                                perconf,
                                mailbox);

    ActorRef::new(uri, cell.clone())
}

struct Guardian<Msg> {
    name: String,
    t: PhantomData<Msg>,
}

impl<Msg: Message> Guardian<Msg> {
    fn new(name: String) -> BoxActor<Msg> {
        let actor = Guardian {
            name: name,
            t: PhantomData
        };

        Box::new(actor)
    }
}

impl<Msg: Message> Actor for Guardian<Msg> {
    type Msg = Msg;

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {}

    fn post_stop(&mut self) {
        trace!("{} guardian stopped", self.name);
    }
}

