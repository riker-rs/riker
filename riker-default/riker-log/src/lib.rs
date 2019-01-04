use std::marker::PhantomData;

use config::Config;
use runtime_fmt::{rt_println, rt_format_args};

use riker::actors::*;
use riker::system::LoggerProps;

pub struct SimpleLogger<Msg: Message> {
    msg: PhantomData<Msg>,
    cfg: LoggerConfig,
}

impl<Msg> Actor for SimpleLogger<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn receive(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Option<ActorRef<Self::Msg>>) {
    }

    fn system_receive(&mut self, _: &Context<Self::Msg>, msg: SystemMsg<Self::Msg>, _: Option<ActorRef<Self::Msg>>) {
        if let SystemMsg::Log(entry) = msg {
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
}

impl<Msg> LoggerProps for SimpleLogger<Msg>
    where Msg: Message
{
    type Msg = Msg;

    fn props(config: &Config) -> BoxActorProd<Self::Msg> {
        Props::new_args(Box::new(SimpleLogger::actor), LoggerConfig::from(config))
    }
}

impl<Msg> SimpleLogger<Msg>
    where Msg: Message
{
    fn actor(cfg: LoggerConfig) -> BoxActor<Msg> {
        let actor = SimpleLogger {
            msg: PhantomData,
            cfg
        };
        Box::new(actor)
    }
}

#[derive(Clone)]
struct LoggerConfig {
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

