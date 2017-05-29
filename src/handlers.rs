//! Handlers
//!  - Endpoint handlers
//!
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::fs;
use std::env;
use std::time;
use std::fmt;

use reqwest;
use url::Url;
use iron::prelude::*;
use iron::{self, status, modifiers};
use router::Router;
use params::Params;
use mime;

use errors::*;


lazy_static! {
    static ref STATIC_ROOT: PathBuf = {
        let mut root = env::current_dir().expect("Failed to get the current directory");
        root.push("static/badges");
        root
    };
    static ref SVG: mime::Mime = "image/svg+xml".parse().unwrap();
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


/// Downloads a fresh badge from shields.io and saves it to `badge_path`
/// Returns the contents of the downloaded file.
///
/// Errors:
///     * Url parse errors from generating a new shields.io url with querystring
///     * Network errors from reqwest
///     * Io errors from copying badge content or writing it to file
fn fetch_badge(badge_type: &Badge, badge_path: PathBuf, name: &str, params: &UrlParams) -> Result<Vec<u8>> {
    println!("[LOG]: fetching fresh badge ({}) -> {:?}", badge_type, badge_path);
    let url = match *badge_type {
        Badge::Crate => format!("https://img.shields.io/crates/v/{krate}.svg", krate=name),
        Badge::Label => format!("https://img.shields.io/badge/{info}.svg", info=name),
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
fn get_badge(badge_type: &Badge, name: &str, params: &UrlParams) -> Result<Vec<u8>> {
    // key for the cache
    let badge_key = params.iter().fold(String::from(name), |mut s, &(ref k, ref v)| {
        s.push('_');
        s.push_str(k);
        s.push('_');
        s.push_str(v);
        s
    });

    let filename = format!("{}__{}.svg", badge_type, badge_key);
    let mut badge_path = PathBuf::from(&*STATIC_ROOT);
    badge_path.push(filename);

    fs::File::open(&badge_path).and_then(|mut file| {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }).or_else(|_| {
        // cached file is missing
        fetch_badge(badge_type, badge_path, name, params)
    })
}


/// Returns the contents of a request badge defined by its `badge_type`, `name`,
/// and modifying `params`
/// If something goes wrong when loading/fetching bytes, redirect to shields.io
fn badge_or_redirect(badge_type: &Badge, name: &str, req: &mut Request) -> IronResult<Response> {
    let params = req.get_ref::<Params>().unwrap()
        .to_strict_map::<String>().unwrap();
    let params: UrlParams = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

    let name = match name.find(".svg") {
        Some(n) => &name[..n],
        None => name,
    };

    match get_badge(badge_type, name, &params) {
        Err(_) => {
            // Failed to fetch a cached or fresh version, redirect to shields.io
            let url = match *badge_type {
                Badge::Crate => format!("https://img.shields.io/crates/v/{krate}.svg?label={krate}", krate=name),
                Badge::Label => format!("https://img.shields.io/badge/{info}.svg?style=social", info=name),
            };
            let url = Url::parse(&url).unwrap();
            let url = iron::Url::from_generic_url(url).unwrap();
            Ok(Response::with((status::Found, modifiers::Redirect(url))))
        }
        Ok(bytes) => {
            Ok(Response::with((SVG.clone(), status::Ok, bytes)))
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

