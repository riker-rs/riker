use std::marker::PhantomData;

use riker::model::Model;
use riker::protocol::Message;
use riker::system::NoIo;
pub use riker_dispatcher::ThreadPoolDispatcher;
pub use riker_log::SimpleLogger;
pub use riker_deadletter::DeadLettersActor;
pub use riker_mapvec::MapVec;
pub use riker_timer::BasicTimer;


#[derive(Clone)]
pub struct DefaultModel<Msg: Message> {
    msg: PhantomData<Msg>,
}

impl<Msg: Message> DefaultModel<Msg> {
    pub fn new() -> Self {
        DefaultModel { msg: PhantomData }
    }
}

impl<Msg: Message> Model for DefaultModel<Msg> {
    type Msg = Msg;
    type Dis = ThreadPoolDispatcher;
    type Ded = DeadLettersActor<Self::Msg>;
    type Tmr = BasicTimer<Self::Msg>;
    type Evs = MapVec<Self::Msg>;
    type Tcp = NoIo<Self::Msg>;
    type Udp = NoIo<Self::Msg>;
    type Log = SimpleLogger<Self::Msg>;
}