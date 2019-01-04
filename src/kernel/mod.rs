mod dispatcher;
mod kernel;
mod kernel_ref;
mod mailbox;
mod provider;
mod queue;

use std::pin::Pin;
use std::sync::mpsc::Sender;

use futures::Future;

pub use self::kernel_ref::KernelRef;
pub use self::kernel::{Kernel, ActorDock, SysActors};
pub use self::dispatcher::Dispatcher;
pub use self::mailbox::{mailbox, run_mailbox, flush_to_deadletters};
pub use self::mailbox::{Mailbox, MailboxSender, MailboxSchedule, MailboxConfig};
pub use self::provider::{BigBang, create_actor_ref};
pub use self::queue::{queue, QueueWriter, QueueReader};

use crate::protocol::Message;
use crate::actor::{ActorRef, ActorId, BoxActor, BoxActorProd, CreateError};
use crate::system::ActorSystem;

#[allow(dead_code)]
pub enum KernelMsg<Msg: Message> {
    Initialize(ActorSystem<Msg>),

    CreateActor(BoxActorProd<Msg>, String, ActorRef<Msg>, Sender<Result<ActorRef<Msg>, CreateError>>),
    TerminateActor(ActorId),
    RestartActor(ActorId),
    ParkActor(ActorId, Option<BoxActor<Msg>>),
    UnparkActor(ActorId),
    RunFuture(Pin<Box<dyn Future<Output=()> + Send + 'static>>),

    Stop,
}