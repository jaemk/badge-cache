//! Service
//!  - Initialize persistent cache
//!  - Initialize cleaning daemon
//!  - Initialize loggers
//!  - Mount url endpoints to `handlers` functions
//!  - Mount static file handler
//!
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fs;

use chrono::{DateTime, UTC};
use time;
use iron::prelude::*;
use iron::status;
use iron::middleware::{BeforeMiddleware, AfterMiddleware};
use iron::headers::{CacheControl, CacheDirective, Expires, HttpDate};
use router::{Router, NoRoute};
use mount::Mount;
use staticfile::Static;
use logger;
use env_logger;

use routes;
use handlers;
use errors::*;


pub static DT_FORMAT: &'static str = "%Y-%m-%d_%H:%M:%S";


pub struct Record {
    pub last_refresh: DateTime<UTC>,
    pub path_buf: PathBuf,
}
impl Record {
    pub fn from_path_buf(pb: &PathBuf) -> Self {
        Self {
            last_refresh: UTC::now(),
            path_buf: pb.clone(),
        }
    }
    pub fn delete_file(self) -> Result<()> {
        fs::remove_file(&self.path_buf)?;
        Ok(())
    }
}


pub type Cache = Arc<Mutex<HashMap<String, Option<Record>>>>;


/// Custom logger to print out access info
struct InfoLog;
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
/// Applies a `Expires: now + 1hr` if no `Expires` header is already set.
struct DefaultCacheSettings;
impl AfterMiddleware for DefaultCacheSettings {
    fn after(&self, _req: &mut Request, mut resp: Response) -> IronResult<Response> {
        if resp.headers.get::<CacheControl>().is_none() {
            resp.headers.set(
                CacheControl(vec![
                    CacheDirective::MaxAge(3600u32), // 1hr
                    CacheDirective::Public,
                ]));
        }
        if resp.headers.get::<Expires>().is_none() {
            resp.headers.set(
                Expires(HttpDate(time::now() + time::Duration::hours(1)))
                );
        }
        Ok(resp)
    }
}


static ERROR_404: &'static str = r##"
<html>
    <pre>
        Nothing to see here... <img src="/badge/~(=^.^)-meow-yellow.svg?style=social"/>
    </pre>
</html>
"##;

/// Custom 404 Error handler/content
struct Error404;
impl AfterMiddleware for Error404 {
    fn catch(&self, _req: &mut Request, e: IronError) -> IronResult<Response> {
        if let Some(_) = e.error.downcast::<NoRoute>() {
            return Ok(Response::with((status::NotFound, mime!(Text/Html), ERROR_404)))
        }
        Err(e)
    }
}


/// Initialize server
pub fn start(host: &str, log_access: bool) {
    // get default host
    let host = if host.is_empty() { "localhost:3000" } else { host };

    // setup our cache
    let cache = Arc::new(Mutex::new(HashMap::new()));

    // initialize cleaning thread
    handlers::init_cleaner(cache.clone());

    // initialize handlers with access to our cache
    let handlers_ = handlers::initialize(cache.clone());

    // mount our url endpoints
    let mut router = Router::new();
    routes::mount(&mut router, &handlers_);

    // Initialize our Chain with our router,
    let mut chain = Chain::new(router);

    // Initialize and link our error loggers and CacheControl Middleware
    env_logger::init().unwrap();
    let (log_before, log_after) = logger::Logger::new(None);
    chain.link_before(log_before);
    chain.link_after(DefaultCacheSettings);
    chain.link_after(log_after);
    chain.link_after(Error404);

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
