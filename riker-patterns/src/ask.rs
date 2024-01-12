#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use futures::channel::oneshot::{channel, Sender as ChannelSender};
use futures::future::RemoteHandle;
use futures::FutureExt;

use riker::actors::*;

/// Convenience fuction to send and receive a message from an actor
///
/// This function sends a message `msg` to the provided actor `receiver`
/// and returns a `Future` which will be completed when `receiver` replies
/// by sending a message to the `sender`. The sender is a temporary actor
/// that fulfills the `Future` upon receiving the reply.
///
/// `futures::future::RemoteHandle` is the future returned and the task
/// is executed on the provided executor `ctx`.
///
/// This pattern is especially useful for interacting with actors from outside
/// of the actor system, such as sending data from HTTP request to an actor
/// and returning a future to the HTTP response, or using await.
///
/// # Examples
///
/// ```
/// # use riker::actors::*;
/// # use riker_patterns::ask::ask;
/// # use futures::future::RemoteHandle;
/// # use futures::executor::block_on;
///
/// #[derive(Default)]
/// struct Reply;
///
/// impl Actor for Reply {
///    type Msg = String;
///
///    fn recv(&mut self,
///                 ctx: &Context<Self::Msg>,
///                 msg: Self::Msg,
///                 sender: Sender) {
///         // reply to the temporary ask actor
///         sender.as_ref().unwrap().try_tell(
///             format!("Hello {}", msg), None
///         ).unwrap();
///     }
/// }
///
/// // set up the actor system
/// let sys = ActorSystem::new().unwrap();
///
/// // create instance of Reply actor
/// let actor = sys.actor_of::<Reply>("reply").unwrap();
///
/// // ask the actor
/// let msg = "Will Riker".to_string();
/// let r: RemoteHandle<String> = ask(&sys, &actor, msg);
///
/// assert_eq!(block_on(r), "Hello Will Riker".to_string());
/// ```

pub fn ask<Msg, Ctx, R, T>(ctx: &Ctx, receiver: &T, msg: Msg) -> RemoteHandle<R>
where
    Msg: Message,
    R: Message,
    Ctx: TmpActorRefFactory + Run,
    T: Tell<Msg>,
{
    let (tx, rx) = channel::<R>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let props = Props::new_from_args(Box::new(AskActor::new), tx);
    let actor = ctx.tmp_actor_of_props(props).unwrap();
    receiver.tell(msg, Some(actor.into()));

    ctx.run(rx.map(|r| r.unwrap())).unwrap()
}

pub fn ask_ref<Msg: Message, R: Message>(
    sys: ActorSystem,
    receiver: &BasicActorRef,
    msg: Msg,
) -> RemoteHandle<R> {
    let (tx, rx) = channel::<R>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let props = Props::new_from_args(Box::new(AskActor::new), tx);
    let actor = sys.tmp_actor_of_props(props).unwrap();
    receiver.try_tell(msg, Some(actor.into())).unwrap();

    sys.run(rx.map(|r| r.unwrap())).unwrap()
}

struct AskActor<Msg> {
    tx: Arc<Mutex<Option<ChannelSender<Msg>>>>,
}

impl<Msg: Message> AskActor<Msg> {
    fn new(tx: Arc<Mutex<Option<ChannelSender<Msg>>>>) -> BoxActor<Msg> {
        let ask = AskActor { tx };
        Box::new(ask)
    }
}

impl<Msg: Message> Actor for AskActor<Msg> {
    type Msg = Msg;

    fn recv(&mut self, ctx: &Context<Msg>, msg: Msg, _: Sender) {
        if let Ok(mut tx) = self.tx.lock() {
            tx.take().unwrap().send(msg).unwrap();
        }
        ctx.stop(&ctx.myself);
    }
}
