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
use std::env;

use chrono::{DateTime, Utc, Local};
use time;
use iron::prelude::*;
use iron::status;
use iron::typemap::Key;
use iron::middleware::AfterMiddleware;
use iron::headers::{CacheControl, CacheDirective, Expires, HttpDate};
use persistent::Read;
use router::{Router, NoRoute};
use mount::Mount;
use staticfile::Static;
use logger;
use env_logger;
use tera::Tera;

use routes;
use handlers;
use errors::*;


pub struct Record {
    pub last_refresh: DateTime<Utc>,
    pub path_buf: PathBuf,
}
impl Record {
    pub fn from_path_buf(pb: &PathBuf) -> Self {
        Self {
            last_refresh: Utc::now(),
            path_buf: pb.clone(),
        }
    }
    pub fn delete_file(self) -> Result<()> {
        fs::remove_file(&self.path_buf)?;
        Ok(())
    }
}


#[derive(Copy, Clone)]
/// Tera template `iron::typemap` type
pub struct Templates;
impl Key for Templates { type Value = Tera; }


/// Alias for our cross thread cache
pub type Cache = Arc<Mutex<HashMap<String, Option<Record>>>>;


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


/// Set ssl cert env. vars to make sure openssl can find required files
fn set_ssl_vars() {
    #[cfg(target_os="linux")]
    {
        if ::std::env::var_os("SSL_CERT_FILE").is_none() {
            ::std::env::set_var("SSL_CERT_FILE", "/etc/ssl/certs/ca-certificates.crt");
        }
        if ::std::env::var_os("SSL_CERT_DIR").is_none() {
            ::std::env::set_var("SSL_CERT_DIR", "/etc/ssl/certs");
        }
    }
}


/// Initialize server
pub fn start(host: &str) {
    set_ssl_vars();

    // get default host
    let host = if host.is_empty() { "localhost:3000" } else { host };

    // Initialize template engine
    let mut tera = compile_templates!("templates/**/*");
    tera.autoescape_on(vec!["html"]);

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

    // Initialize and link:
    // - Loggers
    // - CacheControl Middleware
    // - Custom 404 handler
    // - Persistent template engine access

    // Set a custom logging format & change the env-var to "LOG"
    // e.g. LOG=info badge-cache serve
    env_logger::LogBuilder::new()
        .format(|record| {
            format!("{} [{}] - [{}] -> {}",
                Local::now().format("%Y-%m-%d_%H:%M:%S"),
                record.level(),
                record.location().module_path(),
                record.args()
                )
            })
        .parse(&env::var("LOG").unwrap_or_default())
        .init()
        .expect("failed to initialize logger");

    // iron request-middleware loggers
    let format = logger::Format::new("[{request-time}] [{status}] {method} {uri}").unwrap();
    let (log_before, log_after) = logger::Logger::new(Some(format));

    chain.link_before(log_before);
    chain.link_after(DefaultCacheSettings);
    chain.link_after(log_after);
    chain.link_after(Error404);
    chain.link(Read::<Templates>::both(tera));

    // mount our chain of services and a static file handler
    let mut mount = Mount::new();
    mount.mount("/", chain)
         .mount("/favicon.ico", Static::new(Path::new("static/favicon.ico")))
         .mount("/robots.txt", Static::new(Path::new("static/robots.txt")))
         .mount("/static/", Static::new(Path::new("static")));

    info!(" ** Serving at {} **", host);
    Iron::new(mount).http(host).unwrap();
}
