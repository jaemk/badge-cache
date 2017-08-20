extern crate chrono;
extern crate time;
extern crate clap;
extern crate reqwest;
extern crate url;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate serde_json;

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


/// Construct an `Error::Msg` from a literal or format-string-and-args
macro_rules! format_err {
    ($literal:expr) => {
        Error::from($literal)
    };
    ($literal:expr, $($arg:expr),*) => {
        Error::from(format!($literal, $($arg),*))
    }
}

/// Construct a `return Err(Error::Msg)` from a literal or format-string-and-args
macro_rules! bail {
    ($literal:expr) => {
        return Err(format_err!($literal))
    };
    ($literal:expr, $($arg:expr),*) => {
        return Err(format_err!($literal, $($arg),*))
    }
}


pub mod service;
pub mod handlers;
pub mod routes;
pub mod admin;


#[derive(Debug)]
pub enum Error {
    Nil,
    Msg(String),
    IoError(std::io::Error),
    UrlParseError(url::ParseError),
    Reqwest(reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// So we can glob import `use errors::*;`
pub mod errors {
    pub use super::Error;
    pub use super::Result;
}


use std::fmt;
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

impl<'a> From<&'a str> for Error {
    fn from(s: &'a str) -> Error {
        Error::Msg(String::from(s))
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error::Msg(s)
    }
}

