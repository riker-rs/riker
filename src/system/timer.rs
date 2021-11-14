use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use std::cmp::min;
use std::sync::mpsc::SendError;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    actor::{ActorRef, BasicActorRef, Sender},
    AnyMessage, Message,
};

pub type ScheduleId = Uuid;

pub trait Timer {
    fn schedule<T, M>(
        &self,
        initial_delay: Duration,
        interval: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> ScheduleId
    where
        T: Message + Into<M>,
        M: Message;

    fn schedule_once<T, M>(
        &self,
        delay: Duration,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> ScheduleId
    where
        T: Message + Into<M>,
        M: Message;

    fn schedule_at_time<T, M>(
        &self,
        time: DateTime<Utc>,
        receiver: ActorRef<M>,
        sender: Sender,
        msg: T,
    ) -> ScheduleId
    where
        T: Message + Into<M>,
        M: Message;

    fn cancel_schedule(&self, id: Uuid);
}

pub enum Job {
    Once(OnceJob),
    Repeat(RepeatJob),
    Cancel(Uuid),
    Shutdown,
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

// Default timer implementation using a separate thread that is repeatedly
// parked while waiting for the next job and unparked to either process
// incoming commands or scheduled jobs

pub struct BasicTimer {
    once_jobs: Vec<OnceJob>,
    repeat_jobs: Vec<RepeatJob>,
}

#[derive(Clone)]
pub struct TimerRef {
    thread: std::thread::Thread,
    sender: mpsc::Sender<Job>,
}

impl TimerRef {
    pub fn send(&self, job: Job) -> Result<(), SendError<Job>> {
        let result = self.sender.send(job);
        self.thread.unpark();
        result
    }

    pub(crate) fn stop(&self) {
        self.sender.send(Job::Shutdown).expect("Failed to stop shutdown to timer");
        self.thread.unpark();
    }
}


impl BasicTimer {
    pub fn start() -> TimerRef {
        let mut process = BasicTimer {
            once_jobs: Vec::new(),
            repeat_jobs: Vec::new(),
        };

        let (tx, rx) = mpsc::channel();
        let join_handle = thread::spawn(move || loop {
            // first check whether there are new messages to process
            // in case a new job is scheduled, it should be executed immediately if desired
            if let Ok(job) = rx.try_recv() {
                match job {
                    Job::Cancel(id) => process.cancel(&id),
                    Job::Once(job) => process.schedule_once(job),
                    Job::Repeat(job) => process.schedule_repeat(job),
                    Job::Shutdown => return,
                }
            }

            // a default park timeout duration in case there's nothing else to do
            let mut park_for = Duration::MAX;

            if let Some(time_left) = process.execute_once_jobs() {
                park_for = min(time_left, park_for);
            }
            if let Some(time_left) = process.execute_repeat_jobs() {
                park_for = min(time_left, park_for);
            }

            thread::park_timeout(park_for);
        });

        TimerRef {
            thread: join_handle.thread().clone(),
            sender: tx
        }
    }

    /// Runs all jobs that have been scheduled to run once and which are due
    /// If there are jobs left, return the duration until the next one is due
    fn execute_once_jobs(&mut self) -> Option<Duration> {
        let (send, keep): (Vec<OnceJob>, Vec<OnceJob>) = self
            .once_jobs
            .drain(..)
            .partition(|j| Instant::now() >= j.send_at);

        // send those messages where the 'send_at' time has been reached or elapsed
        for job in send {
            job.send();
        }

        if keep.is_empty() {
            return None;
        }

        // for those messages that are not to be sent yet, just put them back on the vec
        let mut next_job = Duration::MAX;
        let now = Instant::now();
        for job in keep {
            let time_left = job.send_at.duration_since(now);
            next_job = min(next_job, time_left);
            self.once_jobs.push(job);
        }
        Some(next_job)
    }

    /// Executes all repeat jobs that are due and returns the duration until the next is due
    fn execute_repeat_jobs(&mut self) -> Option<Duration> {
        if self.repeat_jobs.is_empty() {
            return None;
        }
        let mut time_left = Duration::MAX;
        let now =  Instant::now();
        for job in self.repeat_jobs.iter_mut() {
            let next_scheduled_run_in = if now >= job.send_at {
                job.send_at = now + job.interval;
                job.send();
                job.interval
            } else {
                job.send_at.duration_since(now)
            };
            time_left = min(next_scheduled_run_in, time_left);
        }
        Some(time_left)
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
