//! Handlers
//!  - Endpoint handlers
//!
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::ffi::OsStr;
use std::fs;
use std::env;
use std::time;
use std::fmt;

use reqwest;
use url::Url;
use iron::prelude::*;
use iron::{self, status, modifiers};
use persistent;
use router::Router;
use params::Params;
use mime;
use chrono::{Duration, UTC};

use service::{Cache, Record};
use errors::*;


lazy_static! {
    static ref STATIC_ROOT: PathBuf = {
        let mut root = env::current_dir().expect("Failed to get the current directory");
        root.push("static/badges");
        root
    };
    static ref LIFESPAN: time::Duration = time::Duration::new(43200, 0);  // 43200s == 12hrs
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


type UrlParams = Vec<(String, String)>;


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
fn fetch_badge(badge_type: &Badge, badge_path: PathBuf, name: &str, filetype: &str, params: &UrlParams) -> Result<Vec<u8>> {
    println!("[LOG]: fetching fresh badge ({}) -> {:?}", badge_type, badge_path);
    let url = match *badge_type {
        Badge::Crate => format!("https://img.shields.io/crates/v/{}.{}", name, filetype),
        Badge::Label => format!("https://img.shields.io/badge/{}.{}", name, filetype),
    };
    let url = Url::parse_with_params(&url, params)?;

    let mut client = reqwest::Client::new()?;
    client.timeout(time::Duration::new(3, 0));
    let mut resp = client.get(url.as_str())
        .form(params)
        .send()?;

    let mut bytes = Vec::new();
    io::copy(&mut resp, &mut bytes)?;

    let mut file = fs::File::create(badge_path)?;
    file.write_all(&bytes)?;

    Ok(bytes)
}


/// Returns bytes of the requested badge
/// Tries to find a cached file, falls back to fetching a fresh badge from shields.io
///
/// Errors:
///     * Io/Url/Reqwest errors from `fetch_badge`
fn get_badge(req: &mut Request, badge_type: &Badge, name: &str, filetype: &str, params: &UrlParams) -> Result<Vec<u8>> {
    // key for the cache
    let mut s = String::from(name);
    s.push_str(filetype);
    let badge_key = params.iter().fold(s, |mut s, &(ref k, ref v)| {
        s.push('_');
        s.push_str(k);
        s.push('_');
        s.push_str(v);
        s
    });

    let cache_mutex = req.get::<persistent::Write<Cache>>().unwrap();

    let filename = format!("{}__{}.{}", badge_type, badge_key, filetype);

    let mut badge_path = PathBuf::from(&*STATIC_ROOT);
    badge_path.push(filename);

    let mut cache = cache_mutex.lock().map_err(|e| Error::Msg(format!("Error acquiring mutex lock: {}", e)))?;
    let record = cache.entry(badge_key).or_insert(None);
    let mut reset_record = false;
    let bytes = match *record {
        None => {
            reset_record = true;
            fetch_badge(badge_type, badge_path, name, &filetype, params)
        }
        Some(ref r) => {
            let twelve_hrs = Duration::from_std(*LIFESPAN).expect("Failed to convert duration");
            if UTC::now().signed_duration_since(r.last_refresh) > twelve_hrs {
                reset_record = true;
                fetch_badge(badge_type, badge_path, name, &filetype, params)
            } else {
                fs::File::open(&badge_path).and_then(|mut file| {
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)?;
                    Ok(bytes)
                }).or_else(|_| {
                    // cached file is missing
                    reset_record = true;
                    fetch_badge(badge_type, badge_path, name, &filetype, params)
                })
            }
        }
    };
    if reset_record { *record = Some(Record::new()); }
    bytes
}


/// Returns the contents of a request badge defined by its `badge_type`, `name`,
/// and modifying `params`
/// If something goes wrong when loading/fetching bytes, redirect to shields.io
fn badge_or_redirect(badge_type: &Badge, name: &str, req: &mut Request) -> IronResult<Response> {
    let params = req.get_ref::<Params>().unwrap()
        .to_strict_map::<String>().unwrap();
    let params: UrlParams = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

    let name = PathBuf::from(name);
    let filetype = name.extension().and_then(OsStr::to_str).unwrap_or("svg");
    let name = name.file_stem().and_then(OsStr::to_str).expect("Failed to extract filename");

    let mimetype = match mime_from_filetype(filetype) {
        Ok(m) => m,
        Err(_) => return Ok(Response::with((status::BadRequest, format!("Invalid filetype: {}. Accepted: [svg, png, jpg, json]", filetype)))),
    };
    match get_badge(req, badge_type, &name, filetype, &params) {
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


/// handle `/crate/:cratename` requests
pub fn krate(req: &mut Request) -> IronResult<Response> {
    // the `:cratename` token should exist or the router messed up
    let crate_name = {
        let crate_name = req.extensions.get::<Router>().unwrap().find("cratename");
        match crate_name {
            Some(name) => name.to_string(),
            None => unreachable!(),
        }
    };
    badge_or_redirect(&Badge::Crate, &crate_name, req)
}


/// handle `/badge/:badgeinfo` requests
pub fn badge(req: &mut Request) -> IronResult<Response> {
    // the `:badgeinfo` token should exist or the router messed up
    let badge_info = {
        let badge_info = req.extensions.get::<Router>().unwrap().find("badgeinfo");
        match badge_info {
            Some(name) => name.to_string(),
            None => unreachable!(),
        }
    };
    badge_or_redirect(&Badge::Label, &badge_info, req)
}

