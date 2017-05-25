//! Handlers
//!  - Endpoint handlers
//!
use std::io::Read;
use std::path::PathBuf;
//use chrono::UTC;

//use serde_json;

use iron::prelude::*;
use iron::{status, Url};
use iron::modifiers;
use router::Router;
use persistent::{Read as PerRead, Write};
use params::Params;

use service::Cache;
use errors::*;


enum BadgeType {
    Crate,
    Badge,
}

fn get_badge(req: &mut Request, badge_type: BadgeType, name: &str) -> Option<PathBuf> {
    let params = req.get_ref::<Params>().unwrap()
        .to_strict_map::<String>().unwrap();
    let badge_key = params.iter().fold(String::from(name), |mut s, (k, v)| {
        s.push_str(&format!("_{}_{}", k, v));
        s
    });
    println!("badge_key: {:?}", badge_key);
    let mutex = req.get::<Write<Cache>>().unwrap();
    let mut cache = mutex.lock().unwrap();
    cache.get(&badge_key).cloned()
}

pub fn home(req: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, "welcome to badge-cache")))
}

pub fn krate(req: &mut Request) -> IronResult<Response> {
    let crate_name = {
        let crate_name = req.extensions.get::<Router>().unwrap().find("cratename");
        match crate_name {
            None => return Ok(Response::with((status::NotFound, "missing crate id"))),
            Some(name) => name.to_string(),
        }
    };
    let badge = get_badge(req, BadgeType::Crate, &crate_name);
    match badge {
        None => {
            let url = format!("https://img.shields.io/crates/v/{krate}.svg?label={krate}", krate=crate_name);
            let url = Url::parse(&url).unwrap();
            return Ok(Response::with((status::Found, modifiers::Redirect(url))))
        }
        _ => (),
    };
    Ok(Response::with((status::Ok, "fetch crate badge")))
}

pub fn badge(_req: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, "fetch generic badge")))
}
