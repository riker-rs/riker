use std::fmt;

use protocol::*;
use actor::{ActorUri, BoxActorProd, ActorCell, CellPublic};
use actor::{Tell, TryTell, SysTell, CreateError, TryMsgError};

/// An actor reference exposes methods to interact with its underlying
/// actor.
/// 
/// All actor references are products of `system.actor_of`
/// or `context.actor_of`. When an actor is created using `actor_of`
/// an `ActorRef` is returned.
/// 
/// Actor references are lightweight and can be cloned without concern
/// for memory use.
/// 
/// In the event that the underlying actor is terminated messages sent
/// to the actor will be routed to dead letters.
/// 
/// If an actor is restarted all existing references continue to
/// function.
#[derive(Clone)]
pub struct ActorRef<Msg: Message> {
    pub uri: ActorUri,
    pub cell: ActorCell<Msg>
}

impl<Msg: Message> ActorRef<Msg> {
    #[doc(hidden)]
    pub fn new(uri: &ActorUri, cell: ActorCell<Msg>) -> ActorRef<Msg> {
        ActorRef {
            uri: uri.clone(),
            cell: cell,
        }
    }

    /// Actor name.
    /// 
    /// Unique among siblings.
    pub fn name(&self) -> &str {
        &self.uri.name.as_str()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    pub fn path(&self) -> &String {
        &self.uri.path
    }

    pub fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    /// Parent reference.
    pub fn parent(&self) -> ActorRef<Msg> {
        self.cell.parent()
    }

    pub fn user_root(&self) -> ActorRef<Msg> {
        self.cell.user_root()
    }

    /// Iterator over children references.
    pub fn children<'a>(&'a self) -> Box<Iterator<Item = ActorRef<Msg>> + 'a> {
        self.cell.children()
    }
}

impl<Msg: Message> Tell for ActorRef<Msg> {
    type Msg = Msg;

    #[allow(unused)]
    fn tell<T>(&self,
                msg: T,
                sender: Option<ActorRef<Self::Msg>>)
        where T: Into<ActorMsg<Self::Msg>>
    {
        let envelope = Envelope {
            msg: msg.into(),
            sender: sender,
        };
        // consume the result (we don't return it to user)
        let _ = self.cell.send_msg(envelope);
    }
}

impl<Msg: Message> TryTell for Option<ActorRef<Msg>> {
    type Msg = Msg;

    fn try_tell<T>(&self,
                msg: T,
                sender: Option<ActorRef<Self::Msg>>) -> Result<(), TryMsgError<T>>
        where T: Into<ActorMsg<Self::Msg>>
    {
        match self {
            Some(ref actor) => {
                let envelope = Envelope {
                    msg: msg.into(),
                    sender: sender,
                };
                let _ = actor.cell.send_msg(envelope);
                Ok(())
            }
            None => Err(TryMsgError::new(msg))
        }
    }
}

impl<Msg: Message> SysTell for ActorRef<Msg> {
    type Msg = Msg;

    fn sys_tell(&self,
                sys_msg: SystemMsg<Msg>,
                sender: Option<ActorRef<Msg>>) {
        let envelope = SystemEnvelope {
            msg: sys_msg,
            sender: sender
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl<Msg: Message> fmt::Debug for ActorRef<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Actor[{:?}]", self.uri)
    }
}

impl<Msg: Message> fmt::Display for ActorRef<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Actor[{}://{}#{}]", self.uri.host, self.uri.path, self.uri.uid)
    }
}

/// Produces `ActorRef`s. `actor_of` blocks on the current thread until
/// the actor has successfully started or failed to start.
/// 
/// It is advised to return from the actor's factory method quickly and
/// handle any initialization in the actor's `pre_start` method, which is
/// invoked after the `ActorRef` is returned.
pub trait ActorRefFactory {
    type Msg: Message;

    fn actor_of(&self, props: BoxActorProd<Self::Msg>, name: &str) -> Result<ActorRef<Self::Msg>, CreateError>;

    fn stop(&self, actor: &ActorRef<Self::Msg>);
}

/// Produces `ActorRef`s under the `temp` guardian actor.
pub trait TmpActorRefFactory {
    type Msg: Message;

    fn tmp_actor_of(&self, props: BoxActorProd<Self::Msg>) -> Result<ActorRef<Self::Msg>, CreateError>;
}

impl<Msg: Message> PartialEq for ActorRef<Msg> {
    fn eq(&self, other: &ActorRef<Msg>) -> bool {
        self.uri.path == other.uri.path
    }
}
