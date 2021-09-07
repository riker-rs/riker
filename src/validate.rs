use std::fmt;

pub fn validate_name(name: &str) -> Result<(), InvalidName> {
    let alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    for c in name.chars() {
        if !alphabet.contains(c) {
            return Err(InvalidName { name: name.into() });
        }
    }

    Ok(())
}

pub struct InvalidName {
    pub name: String,
}

impl fmt::Display for InvalidName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "\"{}\". Invalid name. Must contain only a-Z, 0-9, _, or -",
            self.name
        ))
    }
}

impl fmt::Debug for InvalidName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

pub fn validate_path(path: &str) -> Result<(), InvalidPath> {
    let alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-/.*";
    for c in path.chars() {
        if !alphabet.contains(c) {
            return Err(InvalidPath { path: path.into() });
        }
    }

    Ok(())
}

pub struct InvalidPath {
    path: String,
}

impl fmt::Display for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "\"{}\". Invalid path. Must contain only a-Z, 0-9, /, _, .., - or *",
            self.path
        ))
    }
}

impl fmt::Debug for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}
