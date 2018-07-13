use log;
use config::Config;

use protocol::{Message, SystemMsg};
use actor::{ActorRef, SysTell, BoxActorProd};

#[derive(Clone)]
pub struct Logger<Msg: Message> {
    level: log::Level,
    actor: ActorRef<Msg>,
}

impl<Msg> Logger<Msg>
    where Msg: Message
{
    pub fn init(level: log::Level,
                actor: ActorRef<Msg>) -> Self {
        let logger = Logger {
            level,
            actor
        };

        let _ = log::set_boxed_logger(Box::new(logger.clone())); // result is Err for some reason (.unwrap() panics)
        log::set_max_level(level.to_level_filter());

        logger
    }
}

impl<Msg> log::Log for Logger<Msg>
    where Msg: Message
{
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        self.actor.sys_tell(SystemMsg::Log(LogEntry::from(record)), None);
    }

    fn flush(&self) {
    }
}

pub trait LoggerProps {
    type Msg: Message;

    fn props(config: &Config) -> BoxActorProd<Self::Msg>;
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

pub trait DeadLetterProps {
    type Msg: Message;

    fn props(dl: ActorRef<Self::Msg>) -> BoxActorProd<Self::Msg>;
}
