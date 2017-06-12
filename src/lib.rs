extern crate chrono;
extern crate time;
extern crate clap;
extern crate reqwest;
extern crate url;
#[macro_use] extern crate lazy_static;

#[macro_use] extern crate mime;
#[macro_use] extern crate tera;
extern crate params;

extern crate iron;
extern crate router;
extern crate persistent;
extern crate staticfile;
extern crate mount;
extern crate logger;

#[macro_use] extern crate log;
extern crate env_logger;

pub mod service;
pub mod handlers;
pub mod routes;
pub mod admin;

use std::fmt;


#[derive(Debug)]
pub enum Error {
    Nil,
    Msg(String),
    IoError(std::io::Error),
    UrlParseError(url::ParseError),
    Reqwest(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match *self {
            Nil                      => write!(f, "nothing to see here"),
            Msg(ref s)               => write!(f, "Msg: {}", s),
            IoError(ref e)           => write!(f, "Io: {}", e),
            UrlParseError(ref e)     => write!(f, "UrlParse: {}", e),
            Reqwest(ref e)           => write!(f, "Reqwest: {}", e),
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
