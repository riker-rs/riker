use riker::actors::*;
use riker::actor::All;
use riker::system::DeadLetterProps;
use log::info;

pub struct DeadLettersActor<Msg: Message> {
    dl: ActorRef<Msg>,
}

impl<Msg: Message> DeadLettersActor<Msg> {
    fn new(dl: ActorRef<Msg>) -> BoxActor<Msg> {
        let actor = DeadLettersActor {
            dl,
        };

        Box::new(actor)
    }
}

impl<Msg: Message> Actor for DeadLettersActor<Msg> {
    type Msg = Msg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let msg = ActorMsg::Channel(ChannelMsg::Subscribe(All.into(), ctx.myself()));
        self.dl.tell(msg, None);
    }

    fn other_receive(&mut self, _: &Context<Msg>, msg: ActorMsg<Msg>, _: Option<ActorRef<Msg>>) {
        if let ActorMsg::DeadLetter(dl) = msg {
            info!("DeadLetter: {} => {} ({:?})", dl.sender, dl.recipient, dl.msg)
        }
    }

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {}
}

impl<Msg: Message> DeadLetterProps for DeadLettersActor<Msg> {
    type Msg = Msg;

    fn props(dl: ActorRef<Msg>) -> BoxActorProd<Msg> {
        Props::new_args(Box::new(DeadLettersActor::new), dl)
    }
}

