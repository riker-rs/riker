use std::fmt;
use async_trait::async_trait;

use crate::{
    actor::{
        actor_cell::{ActorCell, ExtendedCell},
        Actor, ActorPath, ActorUri, BoxActorProd, CreateError,
    },
    system::{ActorSystem, SystemMsg},
    AnyMessage, Envelope, Message,
};

#[async_trait]
pub trait ActorReference: Send + Sync {
    /// Actor name.
    ///
    /// Unique among siblings.
    fn name(&self) -> &str;

    /// Actor URI.
    ///
    /// Returns the URI for this actor.
    fn uri(&self) -> &ActorUri;

    /// Actor path.
    ///
    /// e.g. `/user/actor_a/actor_b`
    fn path(&self) -> &ActorPath;

    /// True if this actor is the top level root
    ///
    /// I.e. `/root`
    fn is_root(&self) -> bool;

    /// User root reference
    ///
    /// I.e. `/root/user`
    fn user_root(&self) -> BasicActorRef;

    /// Parent reference
    ///
    /// Returns the `BasicActorRef` of this actor's parent actor
    fn parent(&self) -> BasicActorRef;

    /// True is this actor has any children actors
    fn has_children(&self) -> bool;

    /// True if the given actor is a child of this actor
    fn is_child(&self, actor: &BasicActorRef) -> bool;

    /// Iterator over children references.
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a>;

    /// Send a system message to this actor
    async fn sys_tell(&self, msg: SystemMsg);
}

pub type BoxedTell<T> = Box<dyn Tell<T> + Send + 'static>;

#[async_trait]
pub trait Tell<T>: ActorReference + Send + 'static {
    async fn tell(&self, msg: T, sender: Sender);
    fn box_clone(&self) -> BoxedTell<T>;
}

#[async_trait]
impl<T, M> Tell<T> for ActorRef<M>
where
    T: Message + Into<M>,
    M: Message,
{
    async fn tell(&self, msg: T, sender: Sender) {
        self.send_msg(msg.into(), sender).await;
    }

    fn box_clone(&self) -> BoxedTell<T> {
        Box::new((*self).clone())
    }
}

#[async_trait]
impl<T> ActorReference for BoxedTell<T>
where
    T: Message,
{
    fn name(&self) -> &str {
        (**self).name()
    }

    fn uri(&self) -> &ActorUri {
        (**self).uri()
    }

    fn path(&self) -> &ActorPath {
        (**self).path()
    }

    fn is_root(&self) -> bool {
        (**self).is_root()
    }

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

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        (**self).children()
    }

    async fn sys_tell(&self, msg: SystemMsg) {
        (**self).sys_tell(msg).await
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

/// A lightweight, un-typed reference to interact with its underlying
/// actor instance through concurrent messaging.
///
/// `BasicActorRef` can be derived from an original `ActorRef<Msg>`.
///
/// `BasicActorRef` allows for un-typed messaging using `try_tell`,
/// that will return a `Result`. If the message type was not supported,
/// the result will contain an `Error`.
///
/// `BasicActorRef` can be used when the original `ActorRef` isn't available,
/// when you need to use collections to store references from different actor
/// types, or when using actor selections to message parts of the actor hierarchy.
///
/// In general, it is better to use `ActorRef` where possible.
#[derive(Clone)]
pub struct BasicActorRef {
    pub cell: ActorCell,
}

impl BasicActorRef {
    #[doc(hidden)]
    pub fn new(cell: ActorCell) -> BasicActorRef {
        BasicActorRef { cell }
    }

    pub fn typed<Msg: Message>(&self, cell: ExtendedCell<Msg>) -> ActorRef<Msg> {
        ActorRef { cell }
    }

    pub(crate) async fn sys_init(&self, sys: &ActorSystem) {
        self.cell.kernel().sys_init(sys).await;
    }

    /// Send a message to this actor
    ///
    /// Returns a result. If the message type is not supported Error is returned.
    pub async fn try_tell<Msg>(
        &self,
        msg: Msg,
        sender: impl Into<Option<BasicActorRef>>,
    ) -> Result<(), ()>
    where
        Msg: Message + Send,
    {
        let mut msg = AnyMessage::new(msg, true);
        self.try_tell_any(&mut msg, sender).await
    }

    pub async fn try_tell_any(
        &self,
        msg: &mut AnyMessage,
        sender: impl Into<Option<BasicActorRef>>,
    ) -> Result<(), ()> {
        self.cell.send_any_msg(msg, sender.into()).await
    }
}

#[async_trait]
impl ActorReference for BasicActorRef {
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

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

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    async fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope { msg, sender: None };
        let _ = self.cell.send_sys_msg(envelope).await;
    }
}

#[async_trait]
impl ActorReference for &BasicActorRef {
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

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

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    async fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope { msg, sender: None };
        let _ = self.cell.send_sys_msg(envelope).await;
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
where
    Msg: Message,
{
    fn from(actor: ActorRef<Msg>) -> BasicActorRef {
        BasicActorRef::new(ActorCell::from(actor.cell))
    }
}

impl<Msg> From<ActorRef<Msg>> for Option<BasicActorRef>
where
    Msg: Message,
{
    fn from(actor: ActorRef<Msg>) -> Option<BasicActorRef> {
        Some(BasicActorRef::new(ActorCell::from(actor.cell)))
    }
}

pub type Sender = Option<BasicActorRef>;

/// A lightweight, typed reference to interact with its underlying
/// actor instance through concurrent messaging.
///
/// All ActorRefs are products of `system.actor_of`
/// or `context.actor_of`. When an actor is created using `actor_of`
/// an `ActorRef<Msg>` is returned, where `Msg` is the mailbox
/// message type for the actor.
///
/// Actor references are lightweight and can be cloned without concern
/// for memory use.
///
/// Messages sent to an actor are added to the actor's mailbox.
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
        ActorRef { cell }
    }

    pub async fn send_msg(&self, msg: Msg, sender: impl Into<Option<BasicActorRef>>) {
        let envelope = Envelope {
            msg,
            sender: sender.into(),
        };
        // consume the result (we don't return it to user)
        let _ = self.cell.send_msg(envelope).await;
    }
}

#[async_trait]
impl<Msg: Message> ActorReference for ActorRef<Msg> {
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

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

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    async fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope { msg, sender: None };
        let _ = self.cell.send_sys_msg(envelope).await;
    }
}

#[async_trait]
impl<Msg: Message> ActorReference for &ActorRef<Msg> {
    fn name(&self) -> &str {
        self.cell.uri().name.as_str()
    }

    fn uri(&self) -> &ActorUri {
        self.cell.uri()
    }

    fn path(&self) -> &ActorPath {
        &self.cell.uri().path
    }

    fn is_root(&self) -> bool {
        self.cell.is_root()
    }

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

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = BasicActorRef> + 'a> {
        self.cell.children()
    }

    async fn sys_tell(&self, msg: SystemMsg) {
        let envelope = Envelope { msg, sender: None };
        let _ = self.cell.send_sys_msg(envelope).await;
    }
}

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
#[async_trait]
pub trait ActorRefFactory {
    async fn actor_of<A>(
        &self,
        props: BoxActorProd<A>,
        name: &str,
    ) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor;

    async fn stop<A>(&self, actor: A)
    where
        A: ActorReference;
}

/// Produces `ActorRef`s under the `temp` guardian actor.
#[async_trait]
pub trait TmpActorRefFactory {
    async fn tmp_actor_of<A>(&self, props: BoxActorProd<A>) -> Result<ActorRef<A::Msg>, CreateError>
    where
        A: Actor;
}
