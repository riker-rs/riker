use std::{
    sync::{Arc, Mutex},
    collections::HashSet,
};
use log::trace;

use crate::{
    actor::*,
    actor::actor_cell::{ActorCell, ExtendedCell},
    kernel::{
        kernel::kernel,
        mailbox::mailbox
    },
    system::{
        ActorSystem, SystemMsg,
        system::SysActors
    },
    validate::validate_name
};

#[derive(Clone)]
pub struct Provider {
    inner: Arc<Mutex<ProviderInner>>,
}

struct ProviderInner {
    paths: HashSet<ActorPath>,
    counter: ActorId,
}

impl Provider {
    pub fn new() -> Self {
        let inner = ProviderInner {
                paths: HashSet::new(),
                counter: 100 // ActorIds start at 100
        };

        Provider {
            inner: Arc::new(Mutex::new(inner))
        }
    }

    pub fn create_actor<A>(&self,
                        props: BoxActorProd<A>,
                        name: &str,
                        parent: &BasicActorRef,
                        sys: &ActorSystem) -> Result<ActorRef<A::Msg>, CreateError>
        where A: Actor + 'static
    {
        validate_name(name)?;
        
        let path = ActorPath::new(&format!("{}/{}", parent.path(), name));
        trace!("Attempting to create actor at: {}", path);

        let uid = self.register(&path)?;

        let uri = ActorUri {
            uid,
            path,
            name: Arc::new(name.into()),
            host: sys.host()
        };

        let (sender, sys_sender, mb) =
                                    mailbox::<A::Msg>(sys.sys_settings()
                                    .msg_process_limit);

        let cell = ExtendedCell::new(uri.uid,
                                    uri.clone(),
                                    Some(parent.clone()),
                                    sys,
                                    // None,/*perconf*/
                                    Arc::new(sender.clone()),
                                    sys_sender.clone(),
                                    sender.clone());

        let k = kernel(props, cell.clone(), mb, sys)?;
        let cell = cell.init(&k);

        let actor = ActorRef::new(cell);
        let child = BasicActorRef::from(actor.clone());
        parent.cell.add_child(child);
        actor.sys_tell(SystemMsg::ActorInit);

        Ok(actor)
    }

    fn register(&self, path: &ActorPath) -> Result<ActorId, CreateError> {
        match self.inner.lock() {
            Ok(mut inner) => {
                if inner.paths.contains(path) {
                    return Err(CreateError::AlreadyExists(path.clone()));
                }

                inner.paths.insert(path.clone());
                let id = inner.counter;
                inner.counter += 1;

                Ok(id)
            }
            Err(_) => {
                Err(CreateError::System)
            }
        }
    }

    pub fn unregister(&self, path: &ActorPath) {
        let mut inner = self.inner.lock().unwrap();
        inner.paths.remove(path);
    }
}

pub fn create_root(sys: &ActorSystem) -> SysActors {
    let root = root(sys);

    SysActors {
        root: root.clone(),
        user: guardian(1, "user", "/user", &root, sys),
        sysm: guardian(2, "system", "/system", &root, sys),
        temp: guardian(3, "temp", "/temp", &root, sys)
    }
}

fn root(sys: &ActorSystem) -> BasicActorRef {
    let uri = ActorUri {
        uid: 0,
        name: Arc::new("root".to_string()),
        path: ActorPath::new("/"),
        host: Arc::new("localhost".to_string())
    };
    let (sender, sys_sender, _mb) = mailbox::<SystemMsg>(100);

    // Big bang: all actors have a parent.
    // This means root also needs a parent.
    // An ActorCell, ActorRef and KernelRef are created
    // independently without an actor being created.
    // kernel is just a channel to nowhere
    // let (mut tx, mut _rx) = channel::<KernelMsg>(1000);
    // let bb_k = KernelRef {
    //     tx
    // };

    let bb_cell = ActorCell::new(0,
                                uri.clone(),
                                None,
                                sys,
                                // None, // old perfaconf
                                Arc::new(sender),
                                sys_sender);

    let bigbang = BasicActorRef::new(bb_cell);

    // root
    let props: BoxActorProd<Guardian> = Props::new_args(Box::new(Guardian::new), "root".to_string());
    let (sender, sys_sender, mb) = mailbox::<SystemMsg>(100);

    let cell = ExtendedCell::new(uri.uid,
                                uri.clone(),
                                Some(bigbang.clone()),
                                sys,
                                // None,/*perconf*/
                                Arc::new(sender.clone()),
                                sys_sender.clone(),
                                sender.clone());

    let k = kernel(props, cell.clone(), mb, sys).unwrap();
    let cell = cell.init(&k);
    let actor_ref = ActorRef::new(cell);

    BasicActorRef::from(actor_ref)
}

fn guardian(uid: ActorId,
                name: &str,
                path: &str,
                root: &BasicActorRef,
                sys: &ActorSystem)
                -> BasicActorRef {
    let uri = ActorUri {
        uid,
        name: Arc::new(name.to_string()),
        path: ActorPath::new(path),
        host: Arc::new("localhost".to_string())
    };

    let props: BoxActorProd<Guardian> = Props::new_args(Box::new(Guardian::new), name.to_string());
    let (sender, sys_sender, mb) = mailbox::<SystemMsg>(100);

    let cell = ExtendedCell::new(uri.uid,
                                uri.clone(),
                                Some(root.clone()),
                                sys,
                                // None,/*perconf*/
                                Arc::new(sender.clone()),
                                sys_sender.clone(),
                                sender.clone());

    let k = kernel(props, cell.clone(), mb, sys).unwrap();
    let cell = cell.init(&k);
    let actor_ref = ActorRef::new(cell);

    let actor = BasicActorRef::from(actor_ref);
    root.cell.add_child(actor.clone());
    actor
}

struct Guardian {
    name: String,
}

impl Guardian {
    fn new(name: String) -> Self {
        let actor = Guardian {
            name
        };

        actor
    }
}

impl Actor for Guardian {
    type Msg = SystemMsg;

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<BasicActorRef>) {}

    fn post_stop(&mut self) {
        trace!("{} guardian stopped", self.name);
    }
}
