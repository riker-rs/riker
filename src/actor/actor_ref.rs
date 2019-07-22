use std::fmt;

use crate::{
    actor::{
        Actor, actor_cell::{ActorCell, ExtendedCell}, ActorPath, ActorUri,
        BoxActorProd,
        CreateError,
        props::{ActorFactory, ActorFactoryArgs},
    }, AnyMessage, Envelope,
    Message,
    system::{ActorSystem, SystemMsg},
};

pub trait ActorReference {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str;

    fn uri(&self) -> &ActorUri;

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath;

    fn is_root(&self) -> bool;

    /// Parent reference.
    fn parent(&self) -> BasicActorRef;

    fn user_root(&self) -> BasicActorRef;

    fn has_children(&self) -> bool;

    fn is_child(&self, actor: &BasicActorRef) -> bool;

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a>;

    fn sys_tell(&self, msg: SystemMsg);
}

pub type BoxedTell<T> = Box<dyn Tell<T> + Send + 'static>;

pub trait Tell<T>: ActorReference + Send + 'static {
    fn tell(&self, msg: T, sender: Option<BasicActorRef>);
    fn box_clone(&self) -> BoxedTell<T>;
}

impl<T, M> Tell<T> for ActorRef<M>
    where T: Message + Into<M>, M: Message
{
    fn tell(&self, msg: T, sender: Sender) {
        self.send_msg(msg.into(), sender);
    }

    fn box_clone(&self) -> BoxedTell<T> {
        Box::new((*self).clone())
    }
}

impl<T> ActorReference for BoxedTell<T>
    where T: Message
{
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        (**self).name()
    }

    fn uri(&self) -> &ActorUri {
        (**self).uri()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b
    fn path(&self) -> &ActorPath {
        (**self).path()
    }

    fn is_root(&self) -> bool {
        (**self).is_root()
    }

    /// Parent reference.
    fn parent(&self) -> BasicActorRef {
        (**self).parent()
    }

    fn user_root(&self) -> BasicActorRef {
        (**self).user_root()
    }

    fn has_children(&self) -> bool {
        (**self).has_children()
    }

    fn is_child(&self, actor: &BasicActorRef) -> bool {
        (**self).is_child(actor)
    }

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a> {
        (**self).children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        (**self).sys_tell(msg)
    }
}

impl<T> PartialEq for BoxedTell<T> {
    fn eq(&self, other: &BoxedTell<T>) -> bool {
        self.path() == other.path()
    }
}

impl<T> fmt::Debug for BoxedTell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tell[{:?}]", self.uri())
    }
}

impl<T> fmt::Display for BoxedTell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tell[{}]", self.uri())
    }
}

impl<T: 'static> Clone for BoxedTell<T> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

#[derive(Clone)]
pub struct BasicActorRef {
    pub cell: ActorCell,
}

impl BasicActorRef {
    #[doc(hidden)]
    pub fn new(cell: ActorCell) -> BasicActorRef {
        BasicActorRef {
            cell,
        }
    }

    pub fn typed<Msg: Message>(&self, cell: ExtendedCell<Msg>) -> ActorRef<Msg> {
        ActorRef {
            cell
        }
    }

    pub(crate) fn sys_init(&self, sys: &ActorSystem) {
        self.cell.kernel().sys_init(sys);
    }

    pub fn try_tell<Msg>(&self, msg: Msg,
                         sender: impl Into<Option<BasicActorRef>>)
                         -> Result<(), ()>
        where Msg: Message + Send
    {
        self.try_tell_any(&mut AnyMessage::new(msg, true), sender)
    }

    pub fn try_tell_any(&self, msg: &mut AnyMessage,
                        sender: impl Into<Option<BasicActorRef>>)
                        -> Result<(), ()> {
        self.cell.send_any_msg(msg, sender.into())
    }
}

impl ActorReference for BasicActorRef {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    /// Parent reference.
    fn parent(&self) -> BasicActorRef {
        self.cell.parent()
    }

    fn user_root(&self) -> BasicActorRef {
        self.cell.user_root()
    }

    fn has_children(&self) -> bool {
        self.cell.has_children()
    }

    fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.cell.is_child(actor)
    }

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None,
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl ActorReference for &BasicActorRef {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    /// Parent reference.
    fn parent(&self) -> BasicActorRef {
        self.cell.parent()
    }

    fn user_root(&self) -> BasicActorRef {
        self.cell.user_root()
    }

    fn has_children(&self) -> bool {
        self.cell.has_children()
    }

    fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.cell.is_child(actor)
    }

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None,
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl fmt::Debug for BasicActorRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BasicActorRef[{:?}]", self.cell.uri())
    }
}

impl fmt::Display for BasicActorRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BasicActorRef[{}]", self.cell.uri())
    }
}

impl PartialEq for BasicActorRef {
    fn eq(&self, other: &BasicActorRef) -> bool {
        self.cell.uri().path == other.cell.uri().path
    }
}

impl<Msg> From<ActorRef<Msg>> for BasicActorRef
    where Msg: Message
{
    fn from(actor: ActorRef<Msg>) -> BasicActorRef {
        BasicActorRef::new(ActorCell::from(actor.cell))
    }
}

impl<Msg> From<ActorRef<Msg>> for Option<BasicActorRef>
    where Msg: Message
{
    fn from(actor: ActorRef<Msg>) -> Option<BasicActorRef> {
        Some(BasicActorRef::new(ActorCell::from(actor.cell)))
    }
}

pub type Sender = Option<BasicActorRef>;

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
/// be valid.
#[derive(Clone)]
pub struct ActorRef<Msg: Message> {
    pub cell: ExtendedCell<Msg>,
}

impl<Msg: Message> ActorRef<Msg> {
    #[doc(hidden)]
    pub fn new(cell: ExtendedCell<Msg>) -> ActorRef<Msg> {
        ActorRef {
            cell
        }
    }

    pub fn send_msg(&self,
                    msg: Msg,
                    sender: impl Into<Option<BasicActorRef>>) {
        let envelope = Envelope {
            msg,
            sender: sender.into(),
        };
        // consume the result (we don't return it to user)
        let _ = self.cell.send_msg(envelope);
    }
}

impl<Msg: Message> ActorReference for ActorRef<Msg> {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    /// Parent reference.
    fn parent(&self) -> BasicActorRef {
        self.cell.parent()
    }

    fn user_root(&self) -> BasicActorRef {
        self.cell.user_root()
    }

    fn has_children(&self) -> bool {
        self.cell.has_children()
    }

    fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.cell.is_child(actor)
    }

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None,
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl<Msg: Message> ActorReference for &ActorRef<Msg> {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

    /// Parent reference.
    fn parent(&self) -> BasicActorRef {
        self.cell.parent()
    }

    fn user_root(&self) -> BasicActorRef {
        self.cell.user_root()
    }

    fn has_children(&self) -> bool {
        self.cell.has_children()
    }

    fn is_child(&self, actor: &BasicActorRef) -> bool {
        self.cell.is_child(actor)
    }

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item=BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None,
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

// impl<Msg: Message> TryTell for Option<ActorRef<Msg>> {
//     type Msg = Msg;

//     fn try_tell(&self,
//                 msg: Msg,
//                 sender: impl Into<Option<BasicActorRef>>)
//                 -> Result<(), TryMsgError<Msg>> {
//         match self {
//             Some(ref actor) => {
//                 let envelope = Envelope {
//                     msg,
//                     sender: sender.into(),
//                 };
//                 let _ = actor.cell.send_msg(envelope);
//                 Ok(())
//             }
//             None => Err(TryMsgError::new(msg))
//         }
//     }
// }

impl<Msg: Message> fmt::Debug for ActorRef<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorRef[{:?}]", self.uri())
    }
}

impl<Msg: Message> fmt::Display for ActorRef<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorRef[{}]", self.uri())
    }
}

impl<Msg: Message> PartialEq for ActorRef<Msg> {
    fn eq(&self, other: &ActorRef<Msg>) -> bool {
        self.uri().path == other.uri().path
    }
}


/// Produces `ActorRef`s. `actor_of` blocks on the current thread until
/// the actor has successfully started or failed to start.
/// 
/// It is advised to return from the actor's factory method quickly and
/// handle any initialization in the actor's `pre_start` method, which is
/// invoked after the `ActorRef` is returned.
pub trait ActorRefFactory {
    fn actor_of_props<A>(&self,
                         props: impl Into<BoxActorProd<A>>,
                         name: &str)
                         -> Result<ActorRef<A::Msg>, CreateError>
        where A: Actor;

    fn actor_of<A>(&self, name: &str) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
        where A: ActorFactory + Actor;

    fn actor_of_args<A, Args>(&self, name: &str, args: Args) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
        where A: ActorFactoryArgs<Args>;

    fn stop(&self, actor: impl ActorReference);
}

/// Produces `ActorRef`s under the `temp` guardian actor.
pub trait TmpActorRefFactory {
    fn tmp_actor_of_props<A>(&self,
                             props: impl Into<BoxActorProd<A>>)
                             -> Result<ActorRef<A::Msg>, CreateError>
        where A: Actor;

    fn tmp_actor_of<A>(&self) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
        where A: ActorFactory + Actor;

    fn tmp_actor_of_args<A, Args>(&self, args: Args) -> Result<ActorRef<<A as Actor>::Msg>, CreateError>
        where A: ActorFactoryArgs<Args>;
}
