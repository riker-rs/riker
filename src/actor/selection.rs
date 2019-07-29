use std::iter::Peekable;

use crate::{
    actor::{ActorReference, BasicActorRef, Sender},
    system::SystemMsg,
    validate::{validate_path, InvalidPath},
    Message,
};

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
/// `selection.try_tell()` is used to message actors in the selection.
/// Since a selection is a collection of `BasicActorRef`s messaging is
/// un-typed. Messages not supported by any actor in the selection will
/// be dropped.
#[derive(Debug)]
pub struct ActorSelection {
    anchor: BasicActorRef,
    // dl: BasicActorRef,
    path_vec: Vec<Selection>,
    path: String,
}

impl ActorSelection {
    pub fn new(
        anchor: BasicActorRef,
        // dl: &BasicActorRef,
        path: String,
    ) -> Result<ActorSelection, InvalidPath> {
        validate_path(&path)?;

        let path_vec: Vec<Selection> = path
            .split_terminator('/')
            .map({
                |seg| match seg {
                    ".." => Selection::SelectParent,
                    "*" => Selection::SelectAllChildren,
                    name => Selection::SelectChildName(name.to_string()),
                }
            })
            .collect();

        Ok(ActorSelection {
            anchor,
            // dl: dl.clone(),
            path_vec,
            path,
        })
    }

    pub fn try_tell<Msg>(&self, msg: Msg, sender: impl Into<Option<BasicActorRef>>)
    where
        Msg: Message,
    {
        fn walk<'a, I, Msg>(
            anchor: &BasicActorRef,
            // dl: &BasicActorRef,
            mut path_vec: Peekable<I>,
            msg: Msg,
            sender: &Sender,
            path: &String,
        ) where
            I: Iterator<Item = &'a Selection>,
            Msg: Message,
        {
            let seg = path_vec.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path_vec.peek().is_none() {
                        let parent = anchor.parent();
                        let _ = parent.try_tell(msg, sender.clone());
                    } else {
                        walk(&anchor.parent(), path_vec, msg, sender, path);
                    }
                }
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        let _ = child.try_tell(msg.clone(), sender.clone());
                    }
                }
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({ |c| c.name() == name }).last();
                    if path_vec.peek().is_none() && child.is_some() {
                        let _ = child.unwrap().try_tell(msg, sender.clone());
                    } else if path_vec.peek().is_some() && child.is_some() {
                        walk(
                            &child.as_ref().unwrap(),
                            // dl,
                            path_vec,
                            msg,
                            sender,
                            path,
                        );
                    } else {
                        // todo send to deadletters?
                    }
                }
                None => {}
            }
        }

        walk(
            &self.anchor,
            // &self.dl,
            self.path_vec.iter().peekable(),
            msg,
            &sender.into(),
            &self.path,
        );
    }

    pub fn sys_tell(&self, msg: SystemMsg, sender: impl Into<Option<BasicActorRef>>) {
        fn walk<'a, I>(
            anchor: &BasicActorRef,
            // dl: &BasicActorRef,
            mut path_vec: Peekable<I>,
            msg: SystemMsg,
            sender: &Sender,
            path: &String,
        ) where
            I: Iterator<Item = &'a Selection>,
        {
            let seg = path_vec.next();

            match seg {
                Some(&Selection::SelectParent) => {
                    if path_vec.peek().is_none() {
                        let parent = anchor.parent();
                        parent.sys_tell(msg);
                    } else {
                        walk(&anchor.parent(), path_vec, msg, sender, path);
                    }
                }
                Some(&Selection::SelectAllChildren) => {
                    for child in anchor.children() {
                        child.sys_tell(msg.clone());
                    }
                }
                Some(&Selection::SelectChildName(ref name)) => {
                    let child = anchor.children().filter({ |c| c.name() == name }).last();
                    if path_vec.peek().is_none() && child.is_some() {
                        child.unwrap().sys_tell(msg);
                    } else if path_vec.peek().is_some() && child.is_some() {
                        walk(
                            &child.as_ref().unwrap(),
                            // dl,
                            path_vec,
                            msg,
                            sender,
                            path,
                        );
                    } else {
                        // todo send to deadletters?
                    }
                }
                None => {}
            }
        }

        walk(
            &self.anchor,
            // &self.dl,
            self.path_vec.iter().peekable(),
            msg,
            &sender.into(),
            &self.path,
        );
    }
}

#[derive(Debug)]
enum Selection {
    SelectParent,
    SelectChildName(String),
    SelectAllChildren,
}

pub trait ActorSelectionFactory {
    fn select(&self, path: &str) -> Result<ActorSelection, InvalidPath>;
}
