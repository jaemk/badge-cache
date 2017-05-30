//! Service
//!  - Initialize external and persistent services/structs
//!  - Initialize loggers
//!  - Mount url endpoints to `handlers` functions
//!  - Mount static file handler
//!
use std::path::Path;
use std::collections::HashMap;

use chrono::{DateTime, UTC};
use iron::prelude::*;
use iron::middleware::{BeforeMiddleware, AfterMiddleware};
use iron::headers::{CacheControl, CacheDirective};
use iron::typemap::Key;
use router::Router;
use mount::Mount;
use staticfile::Static;
use persistent::Write;
use logger;
use env_logger;

use routes;


pub static DT_FORMAT: &'static str = "%Y-%m-%d_%H:%M:%S";


pub struct Record {
    pub last_refresh: DateTime<UTC>,
}
impl Record {
    pub fn new() -> Self {
        Self {
            last_refresh: UTC::now(),
        }
    }
}


type CacheStore = HashMap<String, Option<Record>>;


#[derive(Copy, Clone)]
pub struct Cache;
impl Key for Cache { type Value = CacheStore; }


/// Custom logger to print out access info
pub struct InfoLog;
impl BeforeMiddleware for InfoLog {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        let now = UTC::now().format(DT_FORMAT).to_string();
        println!("[{:?}][{}]: {}", req.method, now, req.url);
        Ok(())
    }
    fn catch(&self, req: &mut Request, err: IronError) -> IronResult<()> {
        let now = UTC::now().format(DT_FORMAT).to_string();
        println!("[{:?}][{}]: {} -> {}", req.method, now, req.url, err);
        Err(err)
    }
}


/// Custom `CacheControl` header settings
/// Applies a `Cache-Control: max-age=3600` if no `CacheControl` header is already set.
pub struct DefaultCacheSettings;
impl AfterMiddleware for DefaultCacheSettings {
    fn after(&self, _req: &mut Request, mut resp: Response) -> IronResult<Response> {
        if resp.headers.get::<CacheControl>().is_none() {
            resp.headers.set(
                CacheControl(vec![
                    CacheDirective::MaxAge(3600u32),
                    CacheDirective::Public,
                ]));
        }
        Ok(resp)
    }
}


/// Initialize server
pub fn start(host: &str, log_access: bool) {
    // get default host
    let host = if host.is_empty() { "localhost:3000" } else { host };

    // setup our cache
    let cache = HashMap::new();
    let persistent_cache = Write::<Cache>::both(cache);

    // mount our url endpoints
    let mut router = Router::new();
    routes::mount(&mut router);

    // Initialize our Chain with our router,
    let mut chain = Chain::new(router);

    // Insert our mutable cache into the request.typemap
    chain.link(persistent_cache);

    // Initialize and link our error loggers and CacheControl Middleware
    env_logger::init().unwrap();
    let (log_before, log_after) = logger::Logger::new(None);
    chain.link_before(log_before);
    chain.link_after(DefaultCacheSettings);
    chain.link_after(log_after);

    // Link our access logger if we're logging
    if log_access {
        chain.link_before(InfoLog);
    }

    // mount our chain of services and a static file handler
    let mut mount = Mount::new();
    mount.mount("/", chain)
         .mount("/favicon.ico", Static::new(Path::new("static/favicon.ico")))
         .mount("/static/", Static::new(Path::new("static")));

    println!(" ** Serving at {}", host);
    Iron::new(mount).http(host).unwrap();
}
