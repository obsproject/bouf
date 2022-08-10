use std::error::Error;
use std::fmt;

// We really don't care about descriptive errors too much,
// he program will just panic! for most of then anyway.
#[derive(Debug)]
pub struct SomeError(pub String);

impl fmt::Display for SomeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SomeError {}
