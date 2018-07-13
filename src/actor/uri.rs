use std::hash::{Hash, Hasher};
use std::fmt;
use std::sync::Arc;

use rand;

pub type ActorId = u32;

/// An `ActorUri` represents the location of an actor, including the
/// path and actor system host.
/// 
/// Note: `host` is currently unused but will be utilized when
/// networking and clustering are introduced.
#[derive(Clone)]
pub struct ActorUri {
    pub uid: ActorId,
    pub name: Arc<String>,
    pub path: Arc<String>,
    pub host: Arc<String>,
}

impl ActorUri {
    pub fn new_uid() -> ActorId {
        rand::random::<ActorId>()
    }

    pub fn temp() -> ActorUri {
        ActorUri {
            uid: 0,
            name: Arc::new(String::default()),
            path: Arc::new("/temp/temp_path".to_string()),
            host: Arc::new(String::default()),
        }
    }  
}

impl PartialEq for ActorUri {
    fn eq(&self, other: &ActorUri) -> bool {
        self.path == other.path
    }
}

impl Eq for ActorUri { }

impl Hash for ActorUri {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl fmt::Display for ActorUri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorUri[{}]", self.path)
    }
}

impl fmt::Debug for ActorUri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ActorUri[{}://{}#{}]", self.host, self.path, self.uid)
    }
}
