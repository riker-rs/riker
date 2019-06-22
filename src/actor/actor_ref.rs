use std::fmt;

use crate::{
    Envelope, Message, AnyMessage,
    system::{ActorSystem, SystemMsg},
    actor::{
        Actor, ActorUri, ActorPath, BoxActorProd,
        CreateError, MsgResult, TryMsgError,
        actor_cell::{ActorCell, ExtendedCell}
    }
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a>;

    fn sys_tell(&self, msg: SystemMsg);
}

pub type BoxedTell<T> = Box<dyn Tell<T> + Send + 'static>;

pub trait Tell<T> : ActorReference + Send + 'static {
    fn tell(&self, msg: T, sender: Option<BasicActorRef>);
    fn box_clone(&self) -> BoxedTell<T>;
}

// pub trait TryTell : ActorReference + Send + 'static {
//     fn tell(&self, msg: T, sender: Option<BasicActorRef>);
// }

impl<T: Message + Into<M>, M: Message> Tell<T> for ActorRef<M> {
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        (**self).children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        (**self).sys_tell(msg)
    }
}

impl<T> PartialEq for BoxedTell<T>  {
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
    pub uri: ActorUri, // todo remove and use cell.uri
    pub cell: ActorCell,
}

impl BasicActorRef {
    #[doc(hidden)]
    pub fn new(uri: &ActorUri, cell: ActorCell) -> BasicActorRef {
        BasicActorRef {
            uri: uri.clone(),
            cell,
        }
    }

    pub fn typed<Msg: Message>(&self, cell: ExtendedCell<Msg>) -> ActorRef<Msg> {
        ActorRef {
            uri: self.uri.clone(),
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

        let envelope = Envelope {
            msg: AnyMessage { msg: Box::new(msg) },
            sender: sender.into(),
        };

        self.cell.send_any_msg(envelope)
    }
    // pub fn try_tell(&self, msg: impl Into<AnyMessage>, sender: Sender)
    //             -> Result<(), ()> {

    //     let envelope = Envelope {
    //         msg: msg.into(),
    //         sender: sender.into(),
    //     };

    //     self.cell.send_any_msg(envelope)
    // }
}

impl ActorReference for BasicActorRef {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        &self.uri.name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        &self.uri
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.uri.path
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl ActorReference for &BasicActorRef {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        &self.uri.name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        &self.uri
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.uri.path
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl fmt::Debug for BasicActorRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BasicActorRef[{:?}]", self.uri)
    }
}

impl fmt::Display for BasicActorRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BasicActorRef[{}]", self.uri)
    }
}

impl PartialEq for BasicActorRef {
    fn eq(&self, other: &BasicActorRef) -> bool {
        self.uri.path == other.uri.path
    }
}

impl<Msg> From<ActorRef<Msg>> for BasicActorRef
    where Msg: Message
{
    fn from(actor: ActorRef<Msg>) -> BasicActorRef {
        BasicActorRef::new(&actor.uri, ActorCell::from(actor.cell))
    }
}

impl<Msg> From<ActorRef<Msg>> for Option<BasicActorRef>
    where Msg: Message
{
    fn from(actor: ActorRef<Msg>) -> Option<BasicActorRef> {
        Some(BasicActorRef::new(&actor.uri, ActorCell::from(actor.cell)))
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
    pub uri: ActorUri,
    pub cell: ExtendedCell<Msg>, // todo doesn't cell have uri already?
}

impl<Msg: Message> ActorRef<Msg> {
    #[doc(hidden)]
    pub fn new(uri: &ActorUri,
                cell: ExtendedCell<Msg>) -> ActorRef<Msg> {
        ActorRef {
            uri: uri.clone(),
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
        &self.uri.name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        &self.uri
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.uri.path
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None
        };
        let _ = self.cell.send_sys_msg(envelope);
    }
}

impl<Msg: Message> ActorReference for &ActorRef<Msg> {
    /// Actor name.
    /// 
    /// Unique among siblings.
    fn name(&self) -> &str {
        &self.uri.name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        &self.uri
    }

    /// Actor path.
    /// 
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath {
        &self.uri.path
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
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope {
            msg,
            sender: None
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
        write!(f, "ActorRef[{:?}]", self.uri)
    }
}

impl<Msg: Message> fmt::Display for ActorRef<Msg> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorRef[{}]", self.uri)
    }
}

impl<Msg: Message> PartialEq for ActorRef<Msg> {
    fn eq(&self, other: &ActorRef<Msg>) -> bool {
        self.uri.path == other.uri.path
    }
}



/// Produces `ActorRef`s. `actor_of` blocks on the current thread until
/// the actor has successfully started or failed to start.
/// 
/// It is advised to return from the actor's factory method quickly and
/// handle any initialization in the actor's `pre_start` method, which is
/// invoked after the `ActorRef` is returned.
pub trait ActorRefFactory {
    fn actor_of<A>(&self,
                    props: BoxActorProd<A>,
                    name: &str)
                    -> Result<ActorRef<A::Msg>, CreateError>
    where A: Actor;

    fn stop(&self, actor: impl ActorReference);
}

/// Produces `ActorRef`s under the `temp` guardian actor.
pub trait TmpActorRefFactory {
    fn tmp_actor_of<A>(&self,
                        props: BoxActorProd<A>)
                        -> Result<ActorRef<A::Msg>, CreateError>
    where A: Actor;
}
