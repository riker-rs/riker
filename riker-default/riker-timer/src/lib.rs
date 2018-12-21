use std::time::{Duration, SystemTime};
use std::thread;
use std::sync::mpsc::{channel, Sender};

use config::Config;
use uuid::Uuid;

use riker::protocol::Message;
use riker::system::{TimerFactory, Job, OnceJob, RepeatJob};

impl<Msg: Message> TimerFactory for BasicTimer<Msg> {
    type Msg = Msg;

    fn new(config: &Config, _debug: bool) -> Sender<Job<Msg>> {
        let config = BasicTimerConfig::from(config);
        let tx = BasicTimer::start(config);

        tx
    }
}

pub struct BasicTimer<Msg: Message> {
    once_jobs: Vec<OnceJob<Msg>>,
    repeat_jobs: Vec<RepeatJob<Msg>>,
}

impl<Msg: Message> BasicTimer<Msg> {
    fn start(config: BasicTimerConfig) -> Sender<Job<Msg>> {
        let mut process = BasicTimer {
            once_jobs: Vec::new(),
            repeat_jobs: Vec::new()
        };

        let (tx, rx) = channel();
        thread::spawn(move || {
            loop {
                process.execute_once_jobs();
                process.execute_repeat_jobs();

                if let Ok(job) = rx.try_recv() {
                    match job {
                        Job::Cancel(id) => process.cancel(&id),
                        Job::Once(job) => process.schedule_once(job),
                        Job::Repeat(job) => process.schedule_repeat(job)
                    }
                }

                thread::sleep(Duration::from_millis(config.frequency_millis));
            }
        });

        tx
    }

    fn execute_once_jobs(&mut self) {
        let (send, keep): (Vec<OnceJob<Msg>>, Vec<OnceJob<Msg>>) =
            self.once_jobs.drain(..).partition(|j| SystemTime::now() >= j.send_at);

        // send those messages where the 'send_at' time has been reached or elapsed
        for job in send.into_iter() {
            job.send();
        }

        // for those messages that are not to be sent yet, just put them back on the vec
        for job in keep.into_iter() {
            self.once_jobs.push(job);
        }
    }

    fn execute_repeat_jobs(&mut self) {
        for job in self.repeat_jobs.iter_mut() {
            if SystemTime::now() >= job.send_at {
                job.send_at = SystemTime::now() + job.interval;
                job.send();
            }
        }
    }

    fn cancel(&mut self, id: &Uuid) {
        // slightly sub optimal way of canceling because we don't know the job type
        // so need to do the remove on both collections
        
        if let Some(pos) = self.once_jobs.iter().position(|job| &job.id == id) {
            self.once_jobs.remove(pos);
        }

        if let Some(pos) = self.repeat_jobs.iter().position(|job| &job.id == id) {
            self.repeat_jobs.remove(pos);
        }
    }

    fn schedule_once(&mut self, job: OnceJob<Msg>) {
        if SystemTime::now() >= job.send_at {
            job.send();
        } else {
            self.once_jobs.push(job);
        }
    }

    fn schedule_repeat(&mut self, job: RepeatJob<Msg>) {
        if SystemTime::now() >= job.send_at {
            job.send();
        } else {
            self.repeat_jobs.push(job);
        }
    }
}

struct BasicTimerConfig {
    frequency_millis: u64,
}

impl<'a> From<&'a Config> for BasicTimerConfig {
    fn from(config: &Config) -> Self {
        BasicTimerConfig {
            frequency_millis: config.get_int("scheduler.frequency_millis").unwrap() as u64
        }
    }
}