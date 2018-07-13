use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::net::SocketAddr;
use std::marker::PhantomData;

use config::Config;

use protocol::{Message, ActorMsg, IOMsg};
use actors::{Actor, ActorRef, Context, Tell, BoxActor, BoxActorProd, Props, ActorRefFactory};

pub struct Io<Msg: Message> {
    managers: HashMap<IoType, ActorRef<Msg>>,
}

impl<Msg> Actor for Io<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn other_receive(&mut self, ctx: &Context<Msg>, msg: ActorMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        if let ActorMsg::IO(msg) = msg {
            match msg {
                IOMsg::Manage(io_type, props) => self.register_io_manager(ctx, io_type, props),
                _ => self.foward(msg, sender)
            }
        }
    }

    fn receive(&mut self, _: &Context<Msg>, _: Msg, _: Option<ActorRef<Msg>>) {}
}

impl<Msg> Io<Msg>
    where Msg: Message
{
    pub fn props() -> BoxActorProd<Msg> {
        Props::new(Box::new(Io::actor))
    }

    pub fn actor() -> BoxActor<Msg> {
        let actor = Io {
            managers: HashMap::new()
        };

        Box::new(actor)
    }

    fn register_io_manager(&mut self, ctx: &Context<Msg>, io_type: IoType, props: BoxActorProd<Msg>) {
        let name = String::from(io_type.clone());

        match ctx.actor_of(props, name.as_ref()) {
            Ok(actor) => {
                trace!("Registering IO manager {:?}", io_type);
                self.managers.insert(io_type, actor);
            }
            Err(_) => {
                error!("Failed to register IO manager for {:?}", io_type);
            }
        }
    }

    fn foward(&mut self, msg: IOMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        match msg {
            IOMsg::Connect(io_type, addr) => {
                match self.managers.get(&io_type) {
                    Some(manager) => {
                        manager.tell(IOMsg::Connect(io_type, addr), sender);
                    }
                    None => warn!("No IO manager for type {:?}", io_type)
                }
            }
            IOMsg::Bind(io_type, addr, handler) => {
                match self.managers.get(&io_type) {
                    Some(manager) => 
                    {
                        manager.tell(IOMsg::Bind(io_type, addr, handler), sender);
                    }
                    None => warn!("No IO manager for type {:?}", io_type) // todo handle better
                };
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IoType(String);

impl<'a> From<&'a str> for IoType {
    fn from(t: &str) -> Self {
        IoType(t.to_string())
    }
}

impl Eq for IoType {}

impl Hash for IoType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl From<IoType> for String {
    fn from(t: IoType) -> String {
        t.0
    }
}

impl<'a> From<&'a IoType> for &'a str {
    fn from(t: &'a IoType) -> &'a str {
        t.0.as_ref()
    }
}

// standard types
pub struct Tcp;

impl From<Tcp> for IoType {
    fn from(_: Tcp) -> Self {
        IoType::from("tcp")
    }
}

pub struct Udp; 

impl From<Udp> for IoType {
    fn from(_: Udp) -> Self {
        IoType::from("udp")
    }
}

pub trait IoManagerProps {
    type Msg: Message;

    fn props(config: &Config) -> Option<BoxActorProd<Self::Msg>>;
}


#[derive(Clone, Debug, PartialEq)]
pub struct IoAddress(pub String);

impl FromStr for IoAddress {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let addr = IoAddress(s.to_string());
        Ok(addr)
    }
}

impl From<SocketAddr> for IoAddress {
    fn from(addr: SocketAddr) -> IoAddress {
        IoAddress(format!("{}", addr))
    }
}

pub struct NoIo<Msg: Message> {
    x: PhantomData<Msg>,
}

impl<Msg> IoManagerProps for NoIo<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn props(_: &Config) -> Option<BoxActorProd<Msg>> {
        None
    }
}





