extern crate chrono;
extern crate clap;

#[macro_use] extern crate mime;
extern crate params;

extern crate iron;
extern crate router;
extern crate persistent;
extern crate staticfile;
extern crate mount;


pub mod service;
pub mod handlers;
pub mod routes;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Any(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match *self {
            Any(ref s)       => write!(f, "Error: {}", s),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod errors {
    pub use super::Error;
    pub use super::Result;
}
