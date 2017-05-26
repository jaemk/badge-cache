extern crate chrono;
extern crate clap;
extern crate reqwest;
extern crate url;
#[macro_use] extern crate lazy_static;

#[macro_use] extern crate mime;
extern crate params;

extern crate iron;
extern crate router;
extern crate persistent;
extern crate staticfile;
extern crate mount;
extern crate logger;
extern crate env_logger;

pub mod service;
pub mod handlers;
pub mod routes;

use std::fmt;


#[derive(Debug)]
pub enum Error {
    Any(String),
    IoError(std::io::Error),
    UrlParseError(url::ParseError),
    Reqwest(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match *self {
            Any(ref s)           => write!(f, "Error: {}", s),
            IoError(ref e)       => write!(f, "Error: {}", e),
            UrlParseError(ref e) => write!(f, "Error: {}", e),
            Reqwest(ref e)       => write!(f, "Error: {}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::IoError(error)
    }
}
impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Error {
        Error::UrlParseError(error)
    }
}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Error {
        Error::Reqwest(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod errors {
    pub use super::Error;
    pub use super::Result;
}
