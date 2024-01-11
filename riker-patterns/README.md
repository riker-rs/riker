# Riker Patterns

## Overview

The patterns crate provides a collection of common patterns used in actor systems.

Currently this only includes the Ask pattern and a behavior change pattern that provides a `transform!` macro.

The intention is to add many patterns, some of which are well documented in popular actor programming books, such as [Reactive Messaging Patterns with the Actor Model](https://www.safaribooksonline.com/library/view/reactive-messaging-patterns/9780133846904/).

## Patterns:

### Ask
The Ask pattern allows values to be sent by actors to outside of the actor system. The value is delivered as a `Future`.

Let's look at how this works:
`Cargo.toml`:
```toml
[dependencies]
riker = "0.3.0"
riker-patterns = "0.3.0"
```

`main.rs`:
```rust
use futures::future::RemoteHandle;
use riker_patterns::ask::*;

struct MyActor;

impl Actor for MyActor {
    type Msg = u32;

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {

        // sender is the Ask, waiting to a message to be sent back to it
        sender.try_tell(msg * 2, Some(ctx.myself().into()));
    }
}

fn main() {
    let sys = ActorSystem::new().unwrap();

    let props = Props::new(Box::new(MyActor::new));
    let my_actor = sys.actor_of(props, "my-actor");

    // ask returns a future that automatically is driven
    // to completion by the system.
    let res: RemoteHandle<u32> = ask(&sys, &my_actor, 100);

    // the result future can be passed to a library or fuction that
    // expects a future, or it can be extracted locally using `block_on`.

    let res = block_on(res).unwrap();
    println!("The result value is: {}", res);
}
```

In the background Ask sets up a temporary intermediate actor that lives for the lifetime of the ask. Other actors see this temporary actor as the `sender` and can send a message back to it. When the temporary ask actor receives a message it fulfills the outstanding future and performs a `stop` on itself to cleanup.

Ask is particularly useful when you have part of an application that runs outside of the actor system, or in another actor system, such as a web server (e.g. Hyper) serving API requests. The resulting future can then be chained as part of the future stack.

### Transform

Transform makes changing actor behavior based on its current state easier to reason about. Since actors maintain state, and indeed is a primary concern, being able to handle messages differently based on that state is important. The Transform pattern separates message handling by dedicating a receive function per state. This saves excessive `match`ing to handle several possible states, i.e. handling behavior is pre-empted at the time of state change instead of on each message receive.

!!! info
    If you're familair with Akka on the JVM, `transform` resembles `become`.

Example:

```rust
#[macro_use]
use riker_patterns::transform::*;

impl ShoppingCart {
    // created state
    fn created(&mut self,
                ctx: &Context<MyMsg>,
                msg: MyMsg,
                sender: Sender) {

        match msg {
            MyMsg::AddItem(item) => {
                // add item to cart
                ...
            }
            MyMsg::Cancel => {
                // future messages will be handled by `fn cancelled`
                transform!(self, UserActor::cancelled);
            }
            MyMsg::Checkout => {
                // future messages will be handled by `fn checkout`
                transform!(self, UserActor::checkout);
            }
        }
    }

    // cancelled state
    fn cancelled(&mut self,
                ctx: &Context<MyMsg>,
                msg: MyMsg,
                sender: Sender) {

        match msg {
            MyMsg::AddItem(item) => {
                // can't add item to cancelled cart
                ...
            }
            MyMsg::Cancel => {
                // can't cancel an already cancelled cart
                ...
            }
            MyMsg::Checkout => {
                // can't checkout out a canclled cart
                ...
            }
        }
    }

    // checkout state
    fn checkout(&mut self,
                ctx: &Context<MyMsg>,
                msg: MyMsg,
                sender: Sender) {

        match msg {
            MyMsg::AddItem(item) => {
                // can't add item to cart during checkout
                ...
            }
            MyMsg::Cancel => {
                // future messages will be handled by `fn cancelled`
                transform!(self, UserActor::cancelled);
            }
            MyMsg::Checkout => {
                // cart already in checkout process
                ...
            }
        }
    }
}

impl Actor for ShoppingCart {
    type Msg = MyMsg;

    fn recv(&mut self,
            ctx: &Context<Self::Msg>,
            msg: Self::Msg,
            sender: Sender) {

        // just call the currently set transform function
        (self.rec)(self, ctx, msg, sender)
    }
}
```

The `transform!` macro expects the field name of the current receive function on `self` to be named `rec`. It's easy to use a different name and either use your own macro, or just set the fuction using standard code. The advantage of `transform!` is that it is easy to read and identify when transformation is happening since it is distinct from standard code. 