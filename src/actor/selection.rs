use std::iter::Peekable;
use std::ops::Deref;

use crate::protocol::{Message, SystemMsg, ActorMsg};
use crate::actor::{Tell, SysTell};
use crate::actor::CellInternal;
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
    path: Vec<Selection>,
    path_str: String,
}

impl<Msg: Message> ActorSelection<Msg> {
    pub fn new(anchor: &ActorRef<Msg>, path: &str) -> Result<ActorSelection<Msg>, InvalidPath> {
        validate_path(path)?;

        let (anchor, path_str) = if path.starts_with("/") {
            let anchor = anchor.user_root();
            let anchor_path: String = format!("{}/", anchor.uri.path.deref().clone());
            let path = path.to_string().replace(&anchor_path, "");
            (anchor, path)
        } else {
            (anchor.clone(), path.to_string())
        };

        let path: Vec<Selection> = path_str.split_terminator('/').map({|seg|
            match seg {
                ".." => Selection::SelectParent,
                "*" => Selection::SelectAllChildren,
                name => {
                    Selection::SelectChildName(name.to_string())
                }
            }
        }).collect();

        Ok(ActorSelection { anchor: anchor, path: path, path_str: path_str })
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
                            mut path: Peekable<I>,
                            msg: ActorMsg<Msg>,
                            sender: &Option<ActorRef<Msg>>,
                            path_str: &String)
            where I: Iterator<Item=&'a Selection>, Msg: Message
        {
            let seg = path.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path.peek().is_none() {
                        let parent = anchor.parent();
                        parent.tell(msg, sender.clone());
                    } else {
                        walk(&anchor.parent(), path, msg, sender, path_str);
                    }
                },
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        child.tell(msg.clone(), sender.clone());
                    }
                },
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({|c| c.name() == name}).last();
                    if path.peek().is_none() && child.is_some() {
                        child.unwrap().tell(msg, sender.clone());
                    } else if path.peek().is_some() && child.is_some() {
                        walk(&child.as_ref().unwrap(), path, msg, sender, path_str);
                    } else {
                        let sp = sender.clone().map(|s| s.uri.path.deref().clone());
                        dead_letter(&anchor.cell.dead_letters(),
                                    sp,
                                    path_str.clone(),
                                    msg);
                    }
                },
                None => {}
            }
        }

        walk(&self.anchor, self.path.iter().peekable(), msg.into(), &sender, &self.path_str);
    }
}

impl<Msg: Message> SysTell for ActorSelection<Msg> {
    type Msg = Msg;

    fn sys_tell(&self, msg: SystemMsg<Msg>, sender: Option<ActorRef<Msg>>) {
        fn walk<'a, I, Msg>(anchor: &ActorRef<Msg>,
            mut path: Peekable<I>,
            msg: SystemMsg<Msg>,
            sender: &Option<ActorRef<Msg>>)
            where I: Iterator<Item=&'a Selection>, Msg: Message
        {
            let seg = path.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path.peek().is_none() {
                        let parent = anchor.parent();
                        parent.sys_tell(msg, sender.clone());
                    } else {
                        walk(&anchor.parent(), path, msg, sender);
                    }
                },
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        child.sys_tell(msg.clone(), sender.clone());
                    }
                },
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({|c| c.name() == name}).last();
                    if path.peek().is_none() && child.is_some() {
                        child.unwrap().sys_tell(msg, sender.clone());
                    } else if path.peek().is_some() && child.is_some() {
                        walk(&child.as_ref().unwrap(), path, msg, sender);
                    } else {
                        // dead_letter(&anchor.cell.system().dead_letters(), sender.clone().map(|s| s.path().clone().location.path), path_str.clone(), msg);
                    }
                },
                None => {}
            }
        }

        walk(&self.anchor, self.path.iter().peekable(), msg, &sender);
    }
}
