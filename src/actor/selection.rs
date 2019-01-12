use std::iter::Peekable;
use std::ops::Deref;

use crate::protocol::{Message, SystemMsg, ActorMsg};
use crate::actor::{Tell, SysTell};
use crate::actor::actor_ref::ActorRef;
use crate::actor::channel::dead_letter;
use crate::validate::{InvalidPath, validate_path};

/// A selection represents part of the actor heirarchy, allowing
/// messages to be sent to all actors in the selection.
/// 
/// There are several use cases where you would interact with actors
/// via a selection instead of actor references:
/// 
/// - You know the path of an actor but you don't have its `ActorRef`
/// - You want to broadcast a message to all actors within a path
/// 
/// `ActorRef` is almost always the better choice for actor interaction,
/// since messages are directly sent to the actor's mailbox without
/// any preprocessing or cloning.
/// 
/// `ActorSelection` provides flexibility for the cases where at runtime
/// the `ActorRef`s can't be known. This comes at the cost of traversing
/// part of the actor heirarchy and cloning messages.
/// 
/// A selection is anchored to an `ActorRef` and the path is relative
/// to that actor's path.
/// 
/// If a `selection.tell` results in the message being sent to zero actors,
/// the message is sent to dead letters.
#[derive(Debug)]
pub struct ActorSelection<Msg: Message> {
    anchor: ActorRef<Msg>,
    dl: ActorRef<Msg>,
    path_vec: Vec<Selection>,
    path: String,
}

impl<Msg: Message> ActorSelection<Msg> {
    pub fn new(anchor: ActorRef<Msg>,
                dl: &ActorRef<Msg>,
                path: String) -> Result<ActorSelection<Msg>, InvalidPath> {
        validate_path(&path)?;

        let path_vec: Vec<Selection> = path.split_terminator('/').map({|seg|
            match seg {
                ".." => Selection::SelectParent,
                "*" => Selection::SelectAllChildren,
                name => {
                    Selection::SelectChildName(name.to_string())
                }
            }
        }).collect();

        Ok(ActorSelection {
            anchor,
            dl: dl.clone(),
            path_vec,
            path
        })
    }
}

#[derive(Debug)]
enum Selection {
    SelectParent,
    SelectChildName(String),
    SelectAllChildren,
}

pub trait ActorSelectionFactory {
    type Msg: Message;

    fn select(&self, path: &str) -> Result<ActorSelection<Self::Msg>, InvalidPath>;
}

impl<Msg: Message> Tell for ActorSelection<Msg> {
    type Msg = Msg;

    fn tell<T>(&self,
                msg: T,
                sender: Option<ActorRef<Self::Msg>>)
        where T: Into<ActorMsg<Self::Msg>>
    {
        fn walk<'a, I, Msg>(anchor: &ActorRef<Msg>,
                            dl: &ActorRef<Msg>,
                            mut path_vec: Peekable<I>,
                            msg: ActorMsg<Msg>,
                            sender: &Option<ActorRef<Msg>>,
                            path: &String)
            where I: Iterator<Item=&'a Selection>, Msg: Message
        {
            let seg = path_vec.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path_vec.peek().is_none() {
                        let parent = anchor.parent();
                        parent.tell(msg, sender.clone());
                    } else {
                        walk(&anchor.parent(), dl, path_vec, msg, sender, path);
                    }
                },
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        child.tell(msg.clone(), sender.clone());
                    }
                },
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({|c| c.name() == name}).last();
                    if path_vec.peek().is_none() && child.is_some() {
                        child.unwrap()
                            .tell(msg, sender.clone());
                    } else if path_vec.peek().is_some() && child.is_some() {
                        walk(&child.as_ref().unwrap(),
                            dl,
                            path_vec,
                            msg,
                            sender,
                            path);
                    } else {
                        let sp = sender.clone()
                                        .map(|s| {
                                            s.uri.path.deref().clone()
                                        });
                        dead_letter(dl,
                                    sp,
                                    path.clone(),
                                    msg);
                    }
                },
                None => {}
            }
        }

        walk(&self.anchor,
            &self.dl,
            self.path_vec.iter().peekable(),
            msg.into(),
            &sender,
            &self.path);
    }
}

impl<Msg: Message> SysTell for ActorSelection<Msg> {
    type Msg = Msg;

    fn sys_tell(&self, msg: SystemMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        fn walk<'a, I, Msg>(anchor: &ActorRef<Msg>,
            mut path_vec: Peekable<I>,
            msg: SystemMsg<Msg>,
            sender: &Option<ActorRef<Msg>>)
            where I: Iterator<Item=&'a Selection>, Msg: Message
        {
            let seg = path_vec.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path_vec.peek().is_none() {
                        let parent = anchor.parent();
                        parent.sys_tell(msg, sender.clone());
                    } else {
                        walk(&anchor.parent(), path_vec, msg, sender);
                    }
                },
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        child.sys_tell(msg.clone(), sender.clone());
                    }
                },
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({|c| c.name() == name}).last();
                    if path_vec.peek().is_none() && child.is_some() {
                        child.unwrap().sys_tell(msg, sender.clone());
                    } else if path_vec.peek().is_some() && child.is_some() {
                        walk(&child.as_ref().unwrap(), path_vec, msg, sender);
                    } else {
                        // dead_letter(&anchor.cell.system().dead_letters(), sender.clone().map(|s| s.path().clone().location.path), path_str.clone(), msg);
                    }
                },
                None => {}
            }
        }

        walk(&self.anchor, self.path_vec.iter().peekable(), msg, &sender);
    }
}
