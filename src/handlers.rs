//! Handlers
//!  - Endpoint handlers
//!
use std::io::{self, Read};
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
use persistent::Write;
use params::Params;
use mime;

use service::Cache;
use errors::*;


lazy_static! {
    static ref STATIC_ROOT: PathBuf = {
        let mut root = env::current_dir().expect("Failed to get the current directory");
        root.push("static/badges");
        root
    };
    static ref SVG: mime::Mime = "image/svg+xml".parse().unwrap();
}


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


fn fetch_badge(badge_type: &Badge, badge_key: &str, name: &str, params: &UrlParams, save_dir: &PathBuf) -> Result<PathBuf> {
    println!("[LOG]: fetching fresh badge ({}) -> {}", badge_type, badge_key);
    let (prefix, url)= match *badge_type {
        Badge::Crate => ("crate", format!("https://img.shields.io/crates/v/{krate}.svg", krate=name)),
        Badge::Label => ("label", format!("https://img.shields.io/badge/{info}.svg", info=name)),
    };
    let url = Url::parse_with_params(&url, params)?;

    let mut client = reqwest::Client::new()?;
    client.timeout(time::Duration::new(3, 0));
    let mut resp = client.get(url.as_str())
        .form(params)
        .send()?;

    let fname = format!("{}__{}.svg", prefix, badge_key);
    let mut dest = save_dir.clone();
    dest.push(fname);
    let mut file = fs::File::create(dest.clone())?;
    io::copy(&mut resp, &mut file)?;
    Ok(dest)
}


fn get_badge(req: &mut Request, badge_type: &Badge, name: &str, params: &UrlParams) -> Result<PathBuf> {
    // key for the cache
    let badge_key = params.iter().fold(String::from(name), |mut s, &(ref k, ref v)| {
        s.push_str(&format!("_{}_{}", k, v));
        s
    });

    let mutex = req.get::<Write<Cache>>().unwrap();
    let mut cache = mutex.lock().unwrap();

    let should_save;
    let filepath = match cache.get(&badge_key) {
        None => {
            should_save = true;
            fetch_badge(badge_type, &badge_key, name, params, &*STATIC_ROOT)?
        }
        Some(ref cached) => {
            match fs::File::open(cached) {
                Ok(_) => {
                    should_save = false;
                    cached.to_path_buf()
                }
                Err(_) => {
                    // cached file is missing
                    should_save = true;
                    fetch_badge(badge_type, &badge_key, name, params, &*STATIC_ROOT)?
                }
            }
        }
    };

    if should_save {
        cache.insert(badge_key, filepath.clone());
    }
    Ok(filepath)
}


fn badge_or_redirect(req: &mut Request, badge_type: Badge, name: &str, params: &UrlParams) -> IronResult<Response> {
    match get_badge(req, &badge_type, name, params) {
        Err(_) => {
            // Failed to fetch a cached or fresh version, redirect to shields.io
            let url = match badge_type {
                Badge::Crate => format!("https://img.shields.io/crates/v/{krate}.svg?label={krate}", krate=name),
                Badge::Label => format!("https://img.shields.io/badge/{info}.svg?style=social", info=name),
            };
            let url = Url::parse(&url).unwrap();
            let url = iron::Url::from_generic_url(url).unwrap();
            Ok(Response::with((status::Found, modifiers::Redirect(url))))
        }
        Ok(badge_path) => {
            let mut file = fs::File::open(&badge_path).expect(&format!("failed to open file: {:?}", badge_path));
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).expect(&format!("failed to read file: {:?}", badge_path));
            Ok(Response::with((SVG.clone(), status::Ok, bytes)))
        }
    }

}


pub fn krate(req: &mut Request) -> IronResult<Response> {
    let params = req.get_ref::<Params>().unwrap()
        .to_strict_map::<String>().unwrap();
    let params: UrlParams = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    // the `:cratename` token should exist or the router messed up
    let crate_name = {
        let crate_name = req.extensions.get::<Router>().unwrap().find("cratename");
        match crate_name {
            Some(name) => name.to_string(),
            None => unreachable!(),
        }
    };
    badge_or_redirect(req, Badge::Crate, &crate_name, &params)
}


pub fn badge(req: &mut Request) -> IronResult<Response> {
    let params = req.get_ref::<Params>().unwrap()
        .to_strict_map::<String>().unwrap();
    let params: UrlParams = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    // the `:badgeinfo` token should exist or the router messed up
    let badge_info = {
        let badge_info = req.extensions.get::<Router>().unwrap().find("badgeinfo");
        match badge_info {
            Some(name) => name.to_string(),
            None => unreachable!(),
        }
    };
    badge_or_redirect(req, Badge::Label, &badge_info, &params)
}


pub fn home(_req: &mut Request) -> IronResult<Response> {
    panic!("served by `staticfiles::Static`")
}

