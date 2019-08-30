use std::time::{Duration, SystemTime};
use async_trait::async_trait;
use futures::{channel::mpsc::{unbounded, UnboundedSender, SendError}, prelude::*};
use chrono::{DateTime, Utc};
use config::Config;
use uuid::Uuid;

use crate::{
    actor::{ActorRef, BasicActorRef, Sender},
    AnyMessage, Message,
};
use futures::executor::ThreadPool;


#[derive(Clone)]
pub struct TimerRef(UnboundedSender<Job>);

impl TimerRef {
    pub async fn send(&self, job: Job) ->  Result<(), SendError> {
        let mut tx = self.0.clone();
        tx.send(job).await
    }
}

#[async_trait]
pub trait Timer {
    async fn schedule<T, M>(
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

    async fn schedule_once<T, M>(
        &self,
        delay: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message;

    async fn schedule_at_time<T, M>(
        &self,
        time: DateTime<Utc>,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> Uuid
    where
        T: Message + Into<M>,
        M: Message;

    async fn cancel_schedule(&self, id: Uuid);
}

pub enum Job {
    Once(OnceJob),
    Repeat(RepeatJob),
    Cancel(Uuid),
}

pub struct OnceJob {
    pub id: Uuid,
    pub send_at: SystemTime,
    pub receiver: BasicActorRef,
    pub sender: Sender,
    pub msg: AnyMessage,
}

impl OnceJob {
    pub async fn send(mut self) {
        let _ = self.receiver.try_tell_any(&mut self.msg, self.sender).await;
    }
}

pub struct RepeatJob {
    pub id: Uuid,
    pub send_at: SystemTime,
    pub interval: Duration,
    pub receiver: BasicActorRef,
    pub sender: Sender,
    pub msg: AnyMessage,
}

impl RepeatJob {
    pub async fn send(&mut self) {
        self.receiver
            .try_tell_any(&mut self.msg, self.sender.clone())
            .await
            .unwrap();
    }
}

// Default timer implementation

pub struct BasicTimer {
    once_jobs: Vec<OnceJob>,
    repeat_jobs: Vec<RepeatJob>,
}

impl BasicTimer {
    pub fn start(exec: &mut ThreadPool, cfg: &Config) -> TimerRef {
        let cfg = BasicTimerConfig::from(cfg);

        let mut process = BasicTimer {
            once_jobs: Vec::new(),
            repeat_jobs: Vec::new(),
        };

        let (tx, mut rx) = unbounded();
        exec.spawn_ok(async move {
            loop {
                process.execute_once_jobs().await;
                process.execute_repeat_jobs().await;

                while let Ok(Some(job)) = rx.try_next() {
                    match job {
                        Job::Cancel(id) => process.cancel(&id),
                        Job::Once(job) => process.schedule_once(job).await,
                        Job::Repeat(job) => process.schedule_repeat(job).await,
                    }
                }

                futures_timer::Delay::new(Duration::from_millis(cfg.frequency_millis)).await.unwrap();
            }
        });

        TimerRef(tx)
    }

    pub async fn execute_once_jobs(&mut self) {
        let (send, keep): (Vec<OnceJob>, Vec<OnceJob>) = self
            .once_jobs
            .drain(..)
            .partition(|j| SystemTime::now() >= j.send_at);

        // send those messages where the 'send_at' time has been reached or elapsed
        for job in send.into_iter() {
            job.send().await;
        }

        // for those messages that are not to be sent yet, just put them back on the vec
        for job in keep.into_iter() {
            self.once_jobs.push(job);
        }
    }

    pub async fn execute_repeat_jobs(&mut self) {
        for job in self.repeat_jobs.iter_mut() {
            if SystemTime::now() >= job.send_at {
                job.send_at = SystemTime::now() + job.interval;
                job.send().await;
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

    pub async fn schedule_once(&mut self, job: OnceJob) {
        if SystemTime::now() >= job.send_at {
            job.send().await;
        } else {
            self.once_jobs.push(job);
        }
    }

    pub async fn schedule_repeat(&mut self, mut job: RepeatJob) {
        if SystemTime::now() >= job.send_at {
            job.send().await;
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
