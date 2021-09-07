use super::{
    system::{logger::LoggerConfig, timer::BasicTimerConfig, ThreadPoolConfig},
    kernel::mailbox::MailboxConfig,
};

#[derive(Clone)]
pub struct Config {
    pub debug: bool,
    pub log: LoggerConfig,
    pub mailbox: MailboxConfig,
    pub dispatcher: ThreadPoolConfig,
    pub scheduler: BasicTimerConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            debug: true,
            log: LoggerConfig::default(),
            mailbox: MailboxConfig::default(),
            dispatcher: ThreadPoolConfig::default(),
            scheduler: BasicTimerConfig::default(),
        }
    }
}

impl Config {
    // Option<()> allow to use ? for parsing toml value, ignore it
    fn merge(&mut self, v: &toml::Value) -> Option<()> {
        let v = v.as_table()?;
        let debug = v.get("debug")?.as_bool()?;
        self.debug = debug;
        let log = v.get("log")?;
        self.log.merge(log);
        let mailbox = v.get("mailbox")?;
        self.mailbox.merge(mailbox);
        let dispatcher = v.get("dispatcher")?;
        self.dispatcher.merge(dispatcher);
        let scheduler = v.get("scheduler")?;
        self.scheduler.merge(scheduler);
        None
    }
}

pub fn load_config() -> Config {
    use std::{env, fs::File, io::{self, Read}};

    let mut cfg = Config::default();

    // load the system config
    // riker.toml contains settings for anything related to the actor framework and its modules
    let path = env::var("RIKER_CONF").unwrap_or_else(|_| "config/riker.toml".into());
    let cfg_amendment = File::open(path)
        .and_then(|mut f| {
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            Ok(s)
        })
        .and_then(|s| {
            toml::from_str::<toml::Value>(&s)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        });
    if let Ok(cfg_amendment) = cfg_amendment {
        cfg.merge(&cfg_amendment);
    }

    // TODO: allow app config here?
    // load the user application config
    // app.toml or app.yaml contains settings specific to the user application
    //let path = env::var("APP_CONF").unwrap_or_else(|_| "config/app".into());
    //cfg.merge(File::with_name(&path).required(false)).unwrap();

    cfg
}
