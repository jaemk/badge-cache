//! Service
//!  - Initialize external and persistent services/structs
//!  - Initialize loggers
//!  - Mount url endpoints to `handlers` functions
//!  - Mount static file handler
//!
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use chrono::UTC;
use iron::prelude::*;
use iron::typemap::Key;
use iron::middleware::{BeforeMiddleware};
use router::Router;
use persistent::Write;
use mount::Mount;
use staticfile::Static;
use logger;
use env_logger;

use routes;


static DT_FORMAT: &'static str = "%Y-%m-%d_%H:%M:%S";


type CacheStore = HashMap<String, PathBuf>;


#[derive(Copy, Clone)]
/// Cache wrapper type for iron request type-map
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


pub fn start(host: &str, log_access: bool) {
    // get default host
    let host = if host.is_empty() { "localhost:3000" } else { host };

    // setup our cache
    let cache = HashMap::new();

    // mount our url endpoints
    let mut router = Router::new();
    routes::mount(&mut router);

    // chain our router,
    // insert our mutable cache into request.typemap,
    // initialize and link our loggers if we're logging
    let mut chain = Chain::new(router);
    chain.link(Write::<Cache>::both(cache));

    env_logger::init().unwrap();
    let (log_before, log_after) = logger::Logger::new(None);
    chain.link_before(log_before);
    chain.link_after(log_after);

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
