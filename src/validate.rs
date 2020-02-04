use regex::Regex;
use std::error::Error;
use std::fmt;

pub fn validate_name(name: &str) -> Result<(), InvalidName> {
    let rgx = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    if !rgx.is_match(name) {
        Err(InvalidName { name: name.into() })
    } else {
        Ok(())
    }
}

pub struct InvalidName {
    pub name: String,
}

impl Error for InvalidName {
    fn description(&self) -> &str {
        "Invalid name. Must contain only a-Z, 0-9, _, or -"
    }
}

impl fmt::Display for InvalidName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("\"{}\". {}", self.name, self.description()))
    }
}

impl fmt::Debug for InvalidName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("\"{}\". {}", self.name, self.description()))
    }
}

pub fn validate_path(path: &str) -> Result<(), InvalidPath> {
    let rgx = Regex::new(r"^[a-zA-Z0-9/*._-]+$").unwrap();
    if !rgx.is_match(path) {
        Err(InvalidPath { path: path.into() })
    } else {
        Ok(())
    }
}

pub struct InvalidPath {
    path: String,
}

impl Error for InvalidPath {
    fn description(&self) -> &str {
        "Invalid path. Must contain only a-Z, 0-9, /, _, .., - or *"
    }
}

impl fmt::Display for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("\"{}\". {}", self.path, self.description()))
    }
}

impl fmt::Debug for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("\"{}\". {}", self.path, self.description()))
    }
}
