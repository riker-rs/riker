use tracing::info;

use crate::actor::{
    Actor, ActorFactoryArgs, ActorRef, All, BasicActorRef, ChannelMsg, Context, DeadLetter,
    Subscribe, Tell,
};

/// Simple actor that subscribes to the dead letters channel and logs using the default logger
pub struct DeadLetterLogger {
    dl_chan: ActorRef<ChannelMsg<DeadLetter>>,
}

impl ActorFactoryArgs<ActorRef<ChannelMsg<DeadLetter>>> for DeadLetterLogger {
    fn create_args(dl_chan: ActorRef<ChannelMsg<DeadLetter>>) -> Self {
        DeadLetterLogger { dl_chan }
    }
}

impl Actor for DeadLetterLogger {
    type Msg = DeadLetter;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Box::new(ctx.myself());
        self.dl_chan.tell(
            Subscribe {
                topic: All.into(),
                actor: sub,
            },
            None,
        );
    }

    fn recv(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<BasicActorRef>) {
        info!(
            "DeadLetter: {:?} => {:?} ({:?})", msg.sender, msg.recipient, msg.msg
        )
    }
}
