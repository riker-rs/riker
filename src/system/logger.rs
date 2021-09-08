use crate::actor::{
    Actor, ActorFactoryArgs, ActorRef, All, BasicActorRef, ChannelMsg, Context, DeadLetter,
    Subscribe, Tell,
};
use crate::Config;
use slog::{info, o, Drain, Level, Logger, Never, OwnedKVList, Record};
use std::time::SystemTime;

#[derive(Clone)]
pub struct LoggerConfig {
    time_fmt: String,
    date_fmt: String,
    log_fmt: String,
    filter: Vec<String>,
    level: Level,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        LoggerConfig {
            time_fmt: "%H:%M:%S%:z".to_string(),
            date_fmt: "%Y-%m-%d".to_string(),
            log_fmt: "{date} {time} {level} [{module}] {body}".to_string(),
            filter: vec![],
            level: Level::Debug,
        }
    }
}

impl LoggerConfig {
    // Option<()> allow to use ? for parsing toml value, ignore it
    pub fn merge(&mut self, v: &toml::Value) -> Option<()> {
        let v = v.as_table()?;
        let time_fmt = v.get("time_fmt")?.as_str()?.to_string();
        self.time_fmt = time_fmt;
        let date_fmt = v.get("date_fmt")?.as_str()?.to_string();
        self.date_fmt = date_fmt;
        let log_fmt = v.get("log_fmt")?.as_str()?.to_string();
        self.log_fmt = log_fmt;
        None
    }
}

pub(crate) fn default_log(cfg: &Config) -> Logger {
    let cfg = cfg.log.clone();

    let drain = DefaultConsoleLogger::new(cfg.clone())
        .filter_level(cfg.level)
        .fuse();
    Logger::root(drain, o!())
}

struct DefaultConsoleLogger {
    cfg: LoggerConfig,
}

impl DefaultConsoleLogger {
    fn new(cfg: LoggerConfig) -> Self {
        DefaultConsoleLogger { cfg }
    }
}

impl Drain for DefaultConsoleLogger {
    type Ok = ();
    type Err = Never;

    fn log(&self, record: &Record, _values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let now = SystemTime::now();
        let filter_match = self.cfg.filter.iter().any(|f| record.module().contains(f));
        if !filter_match {
            // note:
            // println! below has replaced rt_println! from runtime-fmt crate.
            // The log format is fixed as "{date} {time} {level} [{module}] {body}".
            // It's not clear if runtime-fmt is maintained any longer as so we'll
            // attempt to find an alternative to provide configurable formatting.
            println!(
                "{:?} {} [{}] {}",
                now.duration_since(SystemTime::UNIX_EPOCH)
                    .expect("system time should be after 1970"),
                record.level().as_short_str(),
                record.module(),
                record.msg()
            );
        }

        Ok(())
    }
}

/// Simple actor that subscribes to the dead letters channel and logs using the default logger
pub struct DeadLetterLogger {
    dl_chan: ActorRef<ChannelMsg<DeadLetter>>,
    logger: Logger,
}

impl ActorFactoryArgs<(ActorRef<ChannelMsg<DeadLetter>>, Logger)> for DeadLetterLogger {
    fn create_args((dl_chan, logger): (ActorRef<ChannelMsg<DeadLetter>>, Logger)) -> Self {
        DeadLetterLogger { dl_chan, logger }
    }
}

impl Actor for DeadLetterLogger {
    type Msg = DeadLetter;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let sub = Box::new(ctx.myself());
        self.dl_chan.tell(
            Subscribe {
                topic: All.into(),
                actor: sub,
            },
            None,
        );
    }

    fn recv(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Option<BasicActorRef>) {
        info!(
            self.logger,
            "DeadLetter: {:?} => {:?} ({:?})", msg.sender, msg.recipient, msg.msg
        )
    }
}
