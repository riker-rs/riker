use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use config::Config;
use uuid::Uuid;

use crate::{
    actor::{ActorRef, BasicActorRef, Sender},
    AnyMessage, Message,
};

pub type TimerRef = mpsc::Sender<Job>;

pub trait Timer {
    fn schedule<T, M>(
        &self,
        initial_delay: Duration,
        interval: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message;

    fn schedule_once<T, M>(
        &self,
        delay: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message;

    fn schedule_at_time<T, M>(
        &self,
        time: DateTime<Utc>,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message;

    fn cancel_schedule(&self, id: Uuid);
}

pub enum Job {
    Once(OnceJob),
    Repeat(RepeatJob),
    Cancel(Uuid),
}

pub struct OnceJob {
    pub id: Uuid,
    pub send_at: Instant,
    pub receiver: BasicActorRef,
    pub sender: Sender,
    pub msg: AnyMessage,
}

impl OnceJob {
    pub fn send(mut self) {
        let _ = self.receiver.try_tell_any(&mut self.msg, self.sender);
    }
}

pub struct RepeatJob {
    pub id: Uuid,
    pub send_at: Instant,
    pub interval: Duration,
    pub receiver: BasicActorRef,
    pub sender: Sender,
    pub msg: AnyMessage,
}

impl RepeatJob {
    pub fn send(&mut self) {
        let _ = self
            .receiver
            .try_tell_any(&mut self.msg, self.sender.clone());
    }
}

// Default timer implementation

pub struct BasicTimer {
    once_jobs: Vec<OnceJob>,
    repeat_jobs: Vec<RepeatJob>,
}

impl BasicTimer {
    pub fn start(cfg: &Config) -> TimerRef {
        let cfg = BasicTimerConfig::from(cfg);

        let mut process = BasicTimer {
            once_jobs: Vec::new(),
            repeat_jobs: Vec::new(),
        };

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || loop {
            process.execute_once_jobs();
            process.execute_repeat_jobs();

            if let Ok(job) = rx.try_recv() {
                match job {
                    Job::Cancel(id) => process.cancel(&id),
                    Job::Once(job) => process.schedule_once(job),
                    Job::Repeat(job) => process.schedule_repeat(job),
                }
            }

            thread::sleep(Duration::from_millis(cfg.frequency_millis));
        });

        tx
    }

    pub fn execute_once_jobs(&mut self) {
        let (send, keep): (Vec<OnceJob>, Vec<OnceJob>) = self
            .once_jobs
            .drain(..)
            .partition(|j| Instant::now() >= j.send_at);

        // send those messages where the 'send_at' time has been reached or elapsed
        for job in send.into_iter() {
            job.send();
        }

        // for those messages that are not to be sent yet, just put them back on the vec
        for job in keep.into_iter() {
            self.once_jobs.push(job);
        }
    }

    pub fn execute_repeat_jobs(&mut self) {
        for job in self.repeat_jobs.iter_mut() {
            if Instant::now() >= job.send_at {
                job.send_at = Instant::now() + job.interval;
                job.send();
            }
        }
    }

    pub fn cancel(&mut self, id: &Uuid) {
        // slightly sub optimal way of canceling because we don't know the job type
        // so need to do the remove on both vecs

        if let Some(pos) = self.once_jobs.iter().position(|job| &job.id == id) {
            self.once_jobs.remove(pos);
        }

        if let Some(pos) = self.repeat_jobs.iter().position(|job| &job.id == id) {
            self.repeat_jobs.remove(pos);
        }
    }

    pub fn schedule_once(&mut self, job: OnceJob) {
        if Instant::now() >= job.send_at {
            job.send();
        } else {
            self.once_jobs.push(job);
        }
    }

    pub fn schedule_repeat(&mut self, mut job: RepeatJob) {
        if Instant::now() >= job.send_at {
            job.send();
        }
        self.repeat_jobs.push(job);
    }
}

struct BasicTimerConfig {
    frequency_millis: u64,
}

impl<'a> From<&'a Config> for BasicTimerConfig {
    fn from(config: &Config) -> Self {
        BasicTimerConfig {
            frequency_millis: config.get_int("scheduler.frequency_millis").unwrap() as u64,
        }
    }
}
