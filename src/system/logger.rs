use log;
use log::{info, Level};
use config::Config;
use runtime_fmt::{rt_println, rt_format_args};

use crate::{
    actor::{
        ActorRef, Tell, BoxActorProd, Props, Actor,
        Context, BasicActorRef,ChannelMsg, All, DeadLetter, Subscribe
    }
};

pub type LogActor = Box<dyn Actor<Msg=LogEntry, Evt=()> + Send>;

#[derive(Clone)]
pub struct Logger {
    level: Level,
    actor: ActorRef<LogEntry>,
}

impl Logger {
    pub fn init(level: Level,
                actor: ActorRef<LogEntry>) -> Self {
        let logger = Logger {
            level,
            actor
        };

        let _ = log::set_boxed_logger(Box::new(logger.clone())); // result is Err for some reason (.unwrap() panics)
        log::set_max_level(level.to_level_filter());

        logger
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        self.actor.tell(LogEntry::from(record), None);
    }

    fn flush(&self) {
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub level: log::Level,
    pub module: Option<String>,
    pub body: String,
}

impl<'a> From<&'a log::Record<'a>> for LogEntry {
    fn from(record: &log::Record) -> Self {
        LogEntry {
            level: record.level(),
            module: record.module_path().map(|m| m.to_string()),
            body: format!("{}", record.args())
        }
    }
}

// default logger
pub struct SimpleLogger {
    cfg: LoggerConfig,
}

impl SimpleLogger {
    pub fn actor(cfg: LoggerConfig) -> LogActor {
        let a = SimpleLogger {
            cfg
        };

        Box::new(a)
    }

    pub fn props(cfg: LoggerConfig) -> BoxActorProd<LogActor> {
        Props::new_args(Box::new(SimpleLogger::actor),
                        cfg)
    }
}

impl Actor for SimpleLogger {
    type Msg = LogEntry;
    type Evt = ();

    fn recv(&mut self,
                _: &Context<LogEntry>,
                entry: LogEntry,
                _: Option<BasicActorRef>) {
        let now = chrono::Utc::now();
        let f_match: Vec<&String> = self.cfg.filter.iter()
            .filter(|f| entry.module.as_ref().map(|m| m.contains(*f)).unwrap_or(false))
            .collect();
        if f_match.is_empty() {
            rt_println!(self.cfg.log_fmt,
                        date = now.format(&self.cfg.date_fmt),
                        time = now.format(&self.cfg.time_fmt),
                        level = entry.level,
                        module = entry.module.unwrap_or_default(),
                        body = entry.body
                        ).unwrap();
        }
    }
}


#[derive(Clone)]
pub struct LoggerConfig {
    time_fmt: String,
    date_fmt: String,
    log_fmt: String,
    filter: Vec<String>,
}

impl<'a> From<&'a Config> for LoggerConfig {
    fn from(config: &Config) -> Self {
        LoggerConfig {
            time_fmt: config.get_str("log.time_format").unwrap().to_string(),
            date_fmt: config.get_str("log.date_format").unwrap().to_string(),
            log_fmt: config.get_str("log.log_format").unwrap().to_string(),
            filter: config.get_array("log.filter").unwrap_or(vec![]).into_iter().map(|e| e.to_string()).collect(),
        }
    }
}

// pub type DLActor = Box<dyn Actor<Msg=DeadLetter, Evt=()> + Send>;

/// Simple actor that subscribes to the dead letters channel and logs using the default logger
pub struct DeadLetterLogger {
    dl_chan: ActorRef<ChannelMsg<DeadLetter>>,
}

impl DeadLetterLogger {
    fn new(dl_chan: ActorRef<ChannelMsg<DeadLetter>>) -> Self {
        DeadLetterLogger {
            dl_chan
        }
    }

    pub fn props(dl_chan: &ActorRef<ChannelMsg<DeadLetter>>)
                    -> BoxActorProd<DeadLetterLogger> {
        
        Props::new_args(Box::new(DeadLetterLogger::new), dl_chan.clone())
    }
}

impl Actor for DeadLetterLogger {
    type Msg = DeadLetter;
    type Evt = ();

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Box::new(ctx.myself());
        self.dl_chan.tell(Subscribe { topic: All.into(), actor: sub }, None);
    }

    fn recv(&mut self,
                _: &Context<Self::Msg>,
                msg: Self::Msg,
                _: Option<BasicActorRef>) {
        info!("DeadLetter: {:?} => {:?} ({:?})", msg.sender, msg.recipient, msg.msg)
    }
}
