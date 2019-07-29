pub(crate) mod kernel;
pub(crate) mod kernel_ref;
pub(crate) mod mailbox;
pub(crate) mod provider;
pub(crate) mod queue;

use crate::system::ActorSystem;

#[allow(dead_code)]
#[derive(Debug)]
pub enum KernelMsg {
    TerminateActor,
    RestartActor,
    RunActor,
    Sys(ActorSystem),
}
