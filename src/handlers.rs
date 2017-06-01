//! Handlers
//!  - Endpoint handlers
//!
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::ffi::OsStr;
use std::fs;
use std::env;
use std::time as std_time;
use std::fmt;
use std::thread;

use reqwest;
use url::Url;
use iron::prelude::*;
use iron::{self, status, modifiers, Handler};
use router::Router;
use params::Params;
use mime;
use chrono::{self, UTC};

use service::{Cache, Record};
use errors::*;


lazy_static! {
    static ref STATIC_ROOT: PathBuf = {
        let mut root = env::current_dir().expect("Failed to get the current directory");
        root.push("static/badges");
        root
    };
    static ref CACHE_LIFESPAN: chrono::Duration = chrono::Duration::seconds(43200);  // 43200s == 12hrs
    static ref CLEAN_INTERVAL: std_time::Duration = std_time::Duration::new(3600, 0);
    static ref SVG: mime::Mime = "image/svg+xml".parse().expect("failed parsing svg mimetype");
    static ref PNG: mime::Mime = "image/png".parse().expect("failed parsing png mimetype");
    static ref JPG: mime::Mime = "image/jpg".parse().expect("failed parsing jpg mimetype");
    static ref JSON: mime::Mime = "application/json".parse().expect("failed parsing json mimetype");
}


/// Badge type represents a `crate` badge or a generic customizable `label` badge
enum Badge {
    Crate,
    Label,
}
impl fmt::Display for Badge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Badge::*;
        match *self {
            Crate => write!(f, "crate"),
            Label => write!(f, "label"),
        }
    }
}


/// `Url::parse_with_params` expects `&[(k, v), ...]`
type UrlParams = Vec<(String, String)>;


/// Cleans up the cache, deleting any expired files.
///
/// Errors:
///     * Unable to acquire mutex lock
fn wait_and_clear(wait_dur: &std_time::Duration, cache: &Cache) -> Result<usize> {
    thread::sleep(*wait_dur);
    let n_removed = {
        let mut cache = cache.lock().map_err(|e| Error::Msg(format!("[Cleaner] Error obtaining cache lock: {}", e)))?;
        let stale: Vec<String> = cache.iter().fold(vec![], |mut stale, (key, record)| {
            if let Some(ref record) = *record {
                // collect stale keys & delete files
                if UTC::now().signed_duration_since(record.last_refresh) > *CACHE_LIFESPAN {
                    stale.push(key.clone());
                    // ignore failed deletions, file may be missing, any skipped files will
                    // be cleaned up by the occasional cron
                    let _ = fs::remove_file(record.path_buf.clone());
                }
            }
            stale
        });
        for key in &stale {
            cache.remove(key);
        }
        stale.len()
    };
    Ok(n_removed)
}


/// Initialize a cleaning thread with cache access
pub fn init_cleaner(cache: Cache) {
    thread::spawn(move ||{
        let mut wait_dur = *CLEAN_INTERVAL;
        loop {
            match wait_and_clear(&wait_dur, &cache) {
                Ok(n) => {
                    wait_dur = *CLEAN_INTERVAL;
                    println!("[Cleaner]: Cleaned and deleted {} stale records", n);
                }
                Err(e) =>  {
                    // cleaner couldn't get a mutex lock, try again in a couple seconds
                    println!("[Cleaner]: {}", e);
                    wait_dur = std_time::Duration::new(30, 0);
                }
            }
        }
    });
}


/// Returns an appropriate mime type per file extension
fn mime_from_filetype(filetype: &str) -> Result<mime::Mime> {
    Ok(match filetype {
        "svg" => SVG.clone(),
        "png" => PNG.clone(),
        "jpg" => JPG.clone(),
        "json" => JSON.clone(),
        _ => return Err(Error::Nil),
    })
}


/// Downloads a fresh badge from shields.io and saves it to `badge_path`
/// Returns the contents of the downloaded file.
///
/// Errors:
///     * Url parse errors from generating a new shields.io url with querystring
///     * Network errors from reqwest
///     * Io errors from copying badge content or writing it to file
fn fetch_badge(badge_type: &Badge, badge_path: &PathBuf, name: &str, filetype: &str, params: &UrlParams) -> Result<Vec<u8>> {
    println!("[LOG]: fetching fresh badge ({}) -> {:?}", badge_type, badge_path);
    let url = match *badge_type {
        Badge::Crate => format!("https://img.shields.io/crates/v/{}.{}", name, filetype),
        Badge::Label => format!("https://img.shields.io/badge/{}.{}", name, filetype),
    };
    let url = Url::parse_with_params(&url, params)?;

    let mut client = reqwest::Client::new()?;
    client.timeout(std_time::Duration::new(3, 0));
    let mut resp = client.get(url.as_str())
        .form(params)
        .send()?;

    let mut bytes = Vec::new();
    io::copy(&mut resp, &mut bytes)?;

    let mut file = fs::File::create(badge_path)?;
    file.write_all(&bytes)?;

    Ok(bytes)
}


fn create_badge_path(badge_type: &Badge, badge_key: &str, filetype: &str) -> PathBuf {
    let filename = format!("{}__{}.{}", badge_type, badge_key, filetype);
    let mut badge_path = PathBuf::from(&*STATIC_ROOT);
    badge_path.push(filename);
    badge_path
}


/// Returns bytes of the requested badge
/// Tries to find a cached file, falls back to fetching a fresh badge from shields.io
///
/// Errors:
///     * Io/Url/Reqwest errors from `fetch_badge`
fn get_badge(cache: Cache, badge_type: &Badge, name: &str, filetype: &str, params: &UrlParams) -> Result<Vec<u8>> {
    // build key for the cache and filename
    let mut s = String::from(name);
    s.push('_');
    s.push_str(filetype);
    let badge_key = params.iter().fold(s, |mut s, &(ref k, ref v)| {
        s.push('_');
        s.push_str(k);
        s.push('_');
        s.push_str(v);
        s
    });

    let mut cache = cache.lock().map_err(|e| Error::Msg(format!("Error acquiring mutex lock: {}", e)))?;

    let record = cache.entry(badge_key.clone()).or_insert(None);
    let mut new_record = None;
    let bytes = match *record {
        None => {
            let new_badge_path = create_badge_path(badge_type, &badge_key, filetype);
            let bytes = fetch_badge(badge_type, &new_badge_path, name, filetype, params);
            new_record = Some(Record::from_path_buf(&new_badge_path));
            bytes
        }
        Some(ref r) => {
            if UTC::now().signed_duration_since(r.last_refresh) > *CACHE_LIFESPAN {
                // content is expired
                let bytes = fetch_badge(badge_type, &r.path_buf, name, filetype, params);
                new_record = Some(Record::from_path_buf(&r.path_buf));
                bytes
            } else {
                // content is still valid
                fs::File::open(&r.path_buf).and_then(|mut file| {
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)?;
                    Ok(bytes)
                }).or_else(|_| {
                    // cached file is missing
                    let bytes = fetch_badge(badge_type, &r.path_buf, name, filetype, params);
                    new_record = Some(Record::from_path_buf(&r.path_buf));
                    bytes
                })
            }
        }
    };
    if let Some(new_record) = new_record {
        *record = Some(new_record);
    }
    bytes
}


/// Returns the contents of a request badge defined by its `badge_type`, `name`,
/// and modifying `params`
/// If something goes wrong when loading/fetching bytes, redirects to shields.io
fn badge_or_redirect(badge_type: &Badge, name: &str, params: &UrlParams, cache: Cache) -> IronResult<Response> {
    let name = PathBuf::from(name);
    let filetype = name.extension().and_then(OsStr::to_str).unwrap_or("svg");
    let name = name.file_stem().and_then(OsStr::to_str).expect("Failed to extract filename");

    let mimetype = match mime_from_filetype(filetype) {
        Ok(m) => m,
        Err(_) => return Ok(Response::with((status::BadRequest, format!("Invalid filetype: {}. Accepted: [svg, png, jpg, json]", filetype)))),
    };
    match get_badge(cache, badge_type, name, filetype, params) {
        Err(_) => {
            // Failed to fetch a cached or fresh version, redirect to shields.io
            let url = match *badge_type {
                Badge::Crate => format!("https://img.shields.io/crates/v/{}.{}", name, filetype),
                Badge::Label => format!("https://img.shields.io/badge/{}.{}", name, filetype),
            };
            let url = Url::parse_with_params(&url, params).expect("invalid params");
            let url = iron::Url::from_generic_url(url).unwrap();
            Ok(Response::with((status::Found, modifiers::Redirect(url))))
        }
        Ok(bytes) => {
            Ok(Response::with((mimetype, status::Ok, bytes)))
        }
    }

}


#[derive(Clone)]
/// handle requests for
/// - /crate/:cratename
/// - /crates/v/:cratename
/// - /badge/:badgeinfo
pub struct BadgeHandler {
    cache: Cache,
}
impl Handler for BadgeHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let (badge_type, name) = {
            let router_params = req.extensions.get::<Router>().expect("failed to extract router params");
            if let Some(crate_name) = router_params.find("cratename") {
                (Badge::Crate, crate_name.to_string())
            } else if let Some(badge_name) = router_params.find("badgeinfo") {
                (Badge::Label, badge_name.to_string())
            } else {
                unreachable!()
            }
        };
        let params = req.get_ref::<Params>().unwrap()
            .to_strict_map::<String>().unwrap();
        let params: UrlParams = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

        badge_or_redirect(&badge_type, &name, &params, self.cache.clone())
    }
}


/// Collection of `struct`s that `impl` `iron::Handler`
pub struct Handlers {
    pub badge_handler: BadgeHandler,
}


/// Return `Handlers` initialized with `Cache` access
pub fn initialize(cache: Cache) -> Handlers {
    Handlers {
        badge_handler: BadgeHandler { cache: cache.clone() },
    }
}
