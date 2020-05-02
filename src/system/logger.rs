use config::Config;
use slog::{info, Drain, Logger, Never, OwnedKVList, Record, Level, o};
use std::str::FromStr;
use crate::actor::{
    Actor, ActorRef, All, BasicActorRef, ChannelMsg, Context, DeadLetter,
    Subscribe, Tell, ActorFactoryArgs,
};


#[derive(Clone)]
pub struct LoggerConfig {
    time_fmt: String,
    date_fmt: String,
    log_fmt: String,
    filter: Vec<String>,
    level: Level
}

impl<'a> From<&'a Config> for LoggerConfig {
    fn from(config: &Config) -> Self {
        LoggerConfig {
            time_fmt: config.get_str("log.time_format").unwrap(),
            date_fmt: config.get_str("log.date_format").unwrap(),
            log_fmt: config.get_str("log.log_format").unwrap(),
            filter: config
                .get_array("log.filter")
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.to_string())
                .collect(),
            level: config
                .get_str("log.level")
                .map(|l| Level::from_str(&l).unwrap_or(Level::Info))
                .unwrap_or(Level::Info)
        }
    }
}

pub(crate) fn default_log(cfg: &Config) -> Logger {
    let cfg = LoggerConfig::from(cfg);

    let drain = DefaultConsoleLogger::new(cfg.clone()).filter_level(cfg.level).fuse();
    let logger = Logger::root(drain, o!());

    let _scope_guard = slog_scope::set_global_logger(logger.clone());
    let _log_guard = slog_stdlog::init();   // will not call `.unwrap()` because this might be called more than once

    logger
}

struct DefaultConsoleLogger {
    cfg: LoggerConfig
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
        let now = chrono::Utc::now();
        let filter_match = self.cfg.filter.iter()
            .any(|f| record.module().contains(f));
        if !filter_match {
            // note:
            // println! below has replaced rt_println! from runtime-fmt crate.
            // The log format is fixed as "{date} {time} {level} [{module}] {body}".
            // It's not clear if runtime-fmt is maintained any longer as so we'll
            // attempt to find an alternative to provide configurable formatting.
            println!("{} {} {} [{}] {}",
                     now.format(&self.cfg.date_fmt),
                     now.format(&self.cfg.time_fmt),
                     record.level().as_short_str(),
                     record.module(),
                     record.msg());
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
        info!(self.logger, "DeadLetter: {:?} => {:?} ({:?})",
            msg.sender, msg.recipient, msg.msg
        )
    }
}
