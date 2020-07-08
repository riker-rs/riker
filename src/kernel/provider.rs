use dashmap::DashMap;
use slog::trace;

use std::sync::Arc;

use crate::system::LoggingSystem;
use crate::{
    actor::actor_cell::{ActorCell, ExtendedCell},
    actor::*,
    kernel::kernel,
    kernel::mailbox::mailbox,
    system::{ActorSystem, SysActors, SystemMsg},
    validate::validate_name,
};

#[derive(Clone)]
pub struct Provider {
    inner: Arc<ProviderInner>,
    log: LoggingSystem,
}

struct ProviderInner {
    paths: DashMap<ActorPath, ()>,
}

impl Provider {
    pub fn new(log: LoggingSystem) -> Self {
        let inner = ProviderInner {
            paths: DashMap::new(),
        };

        Provider {
            inner: Arc::new(inner),
            log,
        }
    }

    pub fn create_actor<A>(
        &self,
        props: BoxActorProd<A>,
        name: &str,
        parent: &BasicActorRef,
        sys: &ActorSystem,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor + 'static,
    {
        validate_name(name)?;

        let path = ActorPath::new(&format!("{}/{}", parent.path(), name));
        trace!(sys.log(), "Attempting to create actor at: {}", path);

        self.register(&path)?;

        let uri = ActorUri {
            path,
            name: Arc::new(name.into()),
            host: sys.host(),
        };

        let (sender, sys_sender, mb) = mailbox::<A::Msg>(sys.sys_settings().msg_process_limit);

        let cell = ExtendedCell::new(
            uri,
            Some(parent.clone()),
            sys,
            // None,/*perconf*/
            Arc::new(sender.clone()),
            sys_sender,
            sender,
        );

        let k = kernel(props, cell.clone(), mb, sys)?;
        let cell = cell.init(&k);

        let actor = ActorRef::new(cell);
        let child = BasicActorRef::from(actor.clone());
        parent.cell.add_child(child);
        actor.sys_tell(SystemMsg::ActorInit);

        Ok(actor)
    }

    fn register(&self, path: &ActorPath) -> Result<(), CreateError> {
        if self.inner.paths.contains_key(path) {
            Err(CreateError::AlreadyExists(path.clone()))
        } else {
            let old = self.inner.paths.replace(path.clone(), ());
            if let Some(old) = old {
                self.inner.paths.replace(old.key().clone(), ());
                Err(CreateError::AlreadyExists(path.clone()))
            } else {
                Ok(())
            }
        }
    }

    pub fn unregister(&self, path: &ActorPath) {
        self.inner.paths.remove(path);
    }
}

pub fn create_root(sys: &ActorSystem) -> SysActors {
    let root = root(sys);

    SysActors {
        user: guardian("user", "/user", &root, sys),
        sysm: guardian("system", "/system", &root, sys),
        temp: guardian("temp", "/temp", &root, sys),
        root,
    }
}

fn root(sys: &ActorSystem) -> BasicActorRef {
    let uri = ActorUri {
        name: Arc::new("root".to_string()),
        path: ActorPath::new("/"),
        host: Arc::new("localhost".to_string()),
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

    let bb_cell = ActorCell::new(
        uri.clone(),
        None,
        sys,
        // None, // old perfaconf
        Arc::new(sender),
        sys_sender,
    );

    let bigbang = BasicActorRef::new(bb_cell);

    // root
    let props: BoxActorProd<Guardian> =
        Props::new_args::<Guardian, _>(("root".to_string(), sys.log()));
    let (sender, sys_sender, mb) = mailbox::<SystemMsg>(100);

    let cell = ExtendedCell::new(
        uri,
        Some(bigbang),
        sys,
        // None,/*perconf*/
        Arc::new(sender.clone()),
        sys_sender,
        sender,
    );

    let k = kernel(props, cell.clone(), mb, sys).unwrap();
    let cell = cell.init(&k);
    let actor_ref = ActorRef::new(cell);

    BasicActorRef::from(actor_ref)
}

fn guardian(
    name: &str,
    path: &str,
    root: &BasicActorRef,
    sys: &ActorSystem,
) -> BasicActorRef {
    let uri = ActorUri {
        name: Arc::new(name.to_string()),
        path: ActorPath::new(path),
        host: Arc::new("localhost".to_string()),
    };

    let props: BoxActorProd<Guardian> =
        Props::new_args::<Guardian, _>((name.to_string(), sys.log()));
    let (sender, sys_sender, mb) = mailbox::<SystemMsg>(100);

    let cell = ExtendedCell::new(
        uri,
        Some(root.clone()),
        sys,
        // None,/*perconf*/
        Arc::new(sender.clone()),
        sys_sender,
        sender,
    );

    let k = kernel(props, cell.clone(), mb, sys).unwrap();
    let cell = cell.init(&k);
    let actor_ref = ActorRef::new(cell);

    let actor = BasicActorRef::from(actor_ref);
    root.cell.add_child(actor.clone());
    actor
}

struct Guardian {
    name: String,
    log: LoggingSystem,
}

impl ActorFactoryArgs<(String, LoggingSystem)> for Guardian {
    fn create_args((name, log): (String, LoggingSystem)) -> Self {
        Guardian { name, log }
    }
}

impl Actor for Guardian {
    type Msg = SystemMsg;

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<BasicActorRef>) {}

    fn post_stop(&mut self) {
        trace!(self.log, "{} guardian stopped", self.name);
    }
}
