extern crate riker;
#[macro_use]
extern crate riker_testkit;
#[macro_use]
extern crate riker_patterns;

use riker::actors::*;
use riker_patterns::transform::Receive;
use riker_testkit::probe::channel::{probe, ChannelProbe};
use riker_testkit::probe::{Probe, ProbeReceive};

// NOTE:
// Transform! will be updated in the near future,
// most likely trait-based.
// This is fairly old currently.

#[derive(Clone, Debug)]
struct TestProbe(ChannelProbe<(), ProbeMsg>);

#[derive(Clone, Debug, PartialEq)]
enum ProbeMsg {
    Ok,
    Err,
}

#[derive(Clone, Debug)]
enum MyMsg {
    SetPassword(String),
    // password
    Authenticate(String),
    // password
    Probe(TestProbe), // a probe will be used
}

#[allow(dead_code)]
struct UserActor {
    username: String,
    password: Option<String>,
    rec: Receive<UserActor, MyMsg>,

    // we need a probe for this test
    probe: Option<TestProbe>,
}

impl ActorFactoryArgs<String> for UserActor {
    fn create_args(username: String) -> Self {
        UserActor {
            username,
            password: None,
            rec: Self::created,
            probe: None,
        }
    }
}

impl UserActor {
    /// Receive method for this actor when it is in a created state
    /// i.e. password has not yet been set.
    fn created(&mut self, _ctx: &Context<MyMsg>, msg: MyMsg, _sender: Sender) {
        match msg {
            MyMsg::SetPassword(passwd) => {
                self.password = Some(passwd);

                // send back a result to sender
                // e.g. `sender.try_tell(Ok, None);`

                // transform behavior to match a state where the *password is set*
                transform!(self, UserActor::active);
            }
            MyMsg::Authenticate(_passwd) => {
                // `MyMsg::Authenticate` is invalid since no user password
                // has been set.
                // Signal that this is an error for the current state
                self.probe.as_ref().unwrap().0.event(ProbeMsg::Err);
            }
            MyMsg::Probe(probe) => {
                // Just because this is a test and probe message is a varient
                self.probe = Some(probe);
            }
        }
    }

    /// Receive method for this actor when a password has been set
    /// and the user account is now active.
    fn active(&mut self, _ctx: &Context<MyMsg>, msg: MyMsg, _sender: Sender) {
        match msg {
            MyMsg::Authenticate(_passwd) => {
                // send back an authentication result to sender
                // e.g. `sender.try_tell(Ok, None);`

                // signal that this is correct
                self.probe.as_ref().unwrap().0.event(ProbeMsg::Ok);
            }
            MyMsg::SetPassword(passwd) => {
                // set a new password
                self.password = Some(passwd);
            }
            _ => {
                // ignore probe
            }
        }
    }
}

impl Actor for UserActor {
    type Msg = MyMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // just call the currently set function
        (self.rec)(self, ctx, msg, sender)
    }
}

#[test]
fn transform() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys
        .actor_of_args::<UserActor, _>("trans", "user123".into())
        .unwrap();

    // set up probe
    let (probe, listen) = probe();
    actor.tell(MyMsg::Probe(TestProbe(probe)), None);

    actor.tell(MyMsg::SetPassword("password123".into()), None);
    actor.tell(MyMsg::Authenticate("password123".into()), None);

    p_assert_eq!(listen, ProbeMsg::Ok);
}

#[test]
fn transform_incorrect() {
    let sys = ActorSystem::new().unwrap();

    let actor = sys
        .actor_of_args::<UserActor, _>("trans", "user123".into())
        .unwrap();

    // set up probe
    let (probe, listen) = probe();
    actor.tell(MyMsg::Probe(TestProbe(probe)), None);

    // in this version we'll try to authenticate the user
    // before a password as been set
    actor.tell(MyMsg::Authenticate("password123".into()), None);

    // we should receive Err.
    p_assert_eq!(listen, ProbeMsg::Err);
}
