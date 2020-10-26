use actix_files::{Files, NamedFile};
use actix_web::{http, rt, web, App, HttpRequest, HttpResponse, HttpServer};
use async_mutex::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tera::{Context, Tera};

use crate::{CONFIG, LOG};

#[derive(Debug, Clone)]
pub struct CachedFile {
    cache_name: String,
    created_millis: u128,
    file_path: PathBuf,
}

lazy_static::lazy_static! {
    pub static ref CACHE: Mutex<HashMap<String, Arc<Mutex<CachedFile>>>> = {
        Mutex::new(HashMap::with_capacity(512))
    };
}

async fn cleanup_cache_dir() -> anyhow::Result<()> {
    use futures::stream::StreamExt;
    slog::info!(LOG, "cleaning cache dir: {}", &CONFIG.cache_dir);
    let reader = tokio::fs::read_dir(&CONFIG.cache_dir).await?;

    reader
        .for_each(|entry| async {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    slog::error!(LOG, "failed unwraping dir entry: {:?}", e);
                    return;
                }
            };
            let path = entry.path();
            if path.is_dir() {
                return;
            }
            let file_name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(e) => {
                    slog::error!(LOG, "failed converting filename to string: {:?}", e);
                    return;
                }
            };
            if file_name == ".gitkeep" {
                return;
            }

            // file names should also be the cache names
            let guard = CACHE.lock().await;
            if guard.get(&file_name).is_none() {
                // If it's been evicted from the cache, then delete the file.
                // This means most things will be deleted on startup.
                slog::info!(LOG, "removing stale cached file: {}, {:?}", file_name, path);
                match tokio::fs::remove_file(&path).await {
                    Ok(_) => (),
                    Err(e) => {
                        slog::error!(LOG, "failed removing stale file: {:?}, {:?}", path, e);
                        return;
                    }
                }
            }
        })
        .await;
    Ok(())
}

async fn cleanup() {
    let start =
        rt::time::Instant::now() + std::time::Duration::from_secs(CONFIG.cleanup_delay_seconds);
    let mut interval = rt::time::interval_at(
        start,
        std::time::Duration::from_secs(CONFIG.cleanup_interval_seconds),
    );
    loop {
        interval.tick().await;
        slog::info!(LOG, "cleaning stale items");

        let now = now_millis();
        let removed_from_cache = {
            let mut cache = CACHE.lock().await;
            let mut to_remove = vec![];
            // can't use ::retain since we need to lock
            // and async mutex for each entry
            for (k, v) in cache.iter() {
                let v = v.lock().await;
                let diff_ms = now - v.created_millis;
                if diff_ms > CONFIG.cache_ttl_millis {
                    slog::info!(LOG, "invalidating cached item: {}", v.cache_name);
                    to_remove.push(k.clone());
                }
            }
            for k in to_remove.iter() {
                cache.remove(k);
            }
            to_remove
        };
        slog::info!(
            LOG,
            "removed {} stale items from cache",
            removed_from_cache.len()
        );
        cleanup_cache_dir()
            .await
            .map_err(|e| {
                slog::error!(LOG, "error cleaning caching dir {:?}", e);
            })
            .ok();
    }
}

async fn index(
    template: web::Data<tera::Tera>,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let s = template
        .render("landing.html", &Context::new())
        .map_err(|_| actix_web::error::ErrorInternalServerError("content error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

async fn reset(
    template: web::Data<tera::Tera>,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let s = template
        .render("reset.html", &Context::new())
        .map_err(|_| actix_web::error::ErrorInternalServerError("content error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[derive(serde::Serialize, Debug)]
enum Kind {
    Crate,
    Badge,
}

#[derive(serde::Serialize)]
struct Params {
    kind: Kind,
    name: String,
    ext: String,
    query_params: String,
    cache_name: String,
    redirect_url: String,
}
impl Params {
    fn new(full_name: &str, kind: Kind, request: &HttpRequest) -> anyhow::Result<Params> {
        let parts = full_name.split('.').collect::<Vec<_>>();
        let (name, ext) = if parts.len() < 2 {
            (full_name.to_string(), CONFIG.default_file_ext.clone())
        } else {
            let parts_len = parts.len();
            let end_ind = parts_len - 1;
            let name = parts[0..end_ind]
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(".");
            let name = if name.len() > CONFIG.max_name_length {
                let (name_head, _) = name.split_at(CONFIG.max_name_length);
                slog::info!(
                    LOG,
                    "name too long {}, truncating to {}: {}",
                    name.len(),
                    CONFIG.max_name_length,
                    name_head
                );
                name_head.to_string()
            } else {
                name
            };

            let ext = parts[end_ind].to_string();
            let ext = if ext.len() > CONFIG.max_ext_length {
                let (ext_head, _) = ext.split_at(CONFIG.max_ext_length);
                slog::info!(
                    LOG,
                    "ext too long {}, truncating to {}: {}",
                    ext.len(),
                    CONFIG.max_ext_length,
                    ext_head
                );
                ext_head.to_string()
            } else {
                ext
            };
            (name, ext)
        };

        let query_params = request.query_string().to_string();
        let query_params = if query_params.len() > CONFIG.max_qs_length {
            let (qs_head, _) = query_params.split_at(CONFIG.max_qs_length);
            slog::info!(
                LOG,
                "query string too long {}, truncating to {}: {}",
                query_params.len(),
                CONFIG.max_qs_length,
                qs_head
            );
            qs_head.to_string()
        } else {
            query_params
        };
        let full_name = if query_params.is_empty() {
            format!("{}.{}", name, ext)
        } else {
            format!("{}.{}?{}", name, ext, query_params)
        };
        let name_for_file = if query_params.is_empty() {
            format!("{}.{}", name, ext)
        } else {
            format!("{}_{}.{}", query_params, name, ext)
        };
        let cache_name = format!("{:?}_{}", kind, name_for_file);

        let base_url = "https://img.shields.io";
        let redirect_url = match kind {
            Kind::Crate => format!("{}/crates/v/{}", base_url, full_name),
            Kind::Badge => format!("{}/badge/{}", base_url, full_name),
        };
        Ok(Params {
            kind,
            name,
            ext,
            query_params,
            cache_name,
            redirect_url,
        })
    }
}

#[derive(Default)]
struct Badge {
    was_cached: bool,
    file_path: Option<PathBuf>,
    redirect_url: String,
}
impl Badge {
    async fn into_response(self, request: &HttpRequest) -> anyhow::Result<HttpResponse> {
        let path = if let Some(p) = self.file_path {
            tokio::fs::metadata(&p).await.map_err(|e| {
                anyhow::anyhow!("path not accessible or doesn't exist: {:?}. {:?}", p, e)
            })?;
            Some(p)
        } else {
            None
        };
        if let Some(p) = path {
            let mut resp = NamedFile::open(p)?
                .into_response(request)
                .map_err(|e| anyhow::anyhow!("asset not found: {:?}", e))?;
            let hdrs = resp.headers_mut();

            let ctrl = http::HeaderValue::from_str(&format!(
                "max-age={}, public",
                CONFIG.http_expiry_seconds
            ))?;
            hdrs.insert(http::header::CACHE_CONTROL, ctrl);

            let expiry_dt = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::seconds(CONFIG.http_expiry_seconds))
                .ok_or_else(|| anyhow::anyhow!("error creating expiry datetime"))?;
            let exp = http::HeaderValue::from_str(&expiry_dt.to_rfc2822())?;
            hdrs.insert(http::header::EXPIRES, exp);
            hdrs.insert(
                http::HeaderName::from_static("x-was-cached"),
                http::HeaderValue::from_str(&format!("{}", self.was_cached))?,
            );
            Ok(resp)
        } else {
            Ok(HttpResponse::TemporaryRedirect()
                .set_header("Location", self.redirect_url)
                .finish())
        }
    }
}

async fn _request_badge_to_file(badge_url: &str, file_path: &Path) -> anyhow::Result<()> {
    slog::info!(
        LOG,
        "requesting fresh badge {} -> {:?}",
        badge_url,
        file_path
    );
    let resp = reqwest::get(badge_url)
        .await
        .map_err(|e| anyhow::anyhow!("request failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("request read failed: {}", e))?;

    use tokio::io::AsyncWriteExt;
    let mut f = tokio::fs::File::create(file_path)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create file {}", e))?;
    f.write_all(&resp)
        .await
        .map_err(|e| anyhow::anyhow!("failed writing response to file {}", e))?;
    Ok(())
}

fn now_millis() -> u128 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|dur| dur.as_millis())
        .unwrap_or(0)
}

async fn _get_cached_badge(params: &Params) -> anyhow::Result<(bool, PathBuf)> {
    //  generate new cache values
    let file_path = Path::new(&CONFIG.cache_dir).join(&params.cache_name);
    let new_created_millis = now_millis();
    let new_inner = Arc::new(Mutex::new(CachedFile {
        cache_name: params.cache_name.clone(),
        created_millis: new_created_millis,
        file_path: file_path.clone(),
    }));

    // lock the cache and get or insert
    let mut cache = CACHE.lock().await;
    let inner = cache
        .entry(params.cache_name.clone())
        .or_insert_with(|| new_inner.clone());

    // clone the inner pointer and lock the individual entry
    // while we're still holding the cache lock.
    let owned_inner = inner.clone();
    let locked_inner = owned_inner.lock().await;

    // we've got a cached value if it doesn't match our new insertion timestamp
    let is_cached = locked_inner.created_millis != new_created_millis;
    let is_cached = if is_cached {
        // and if it hasn't expired
        let now = now_millis();
        let diff = now - locked_inner.created_millis;
        if diff > CONFIG.cache_ttl_millis {
            // if it did expire, swap the existing thing for our new entry
            slog::info!(LOG, "cached badge expired: {}", params.cache_name);
            *inner = new_inner.clone();
            false
        } else {
            true
        }
    } else {
        false
    };

    // drop the lock on the cache as a whole - we've still got the
    // lock on the individual entry so no one else can be retrieving
    // and saving this badge at the same time.
    std::mem::drop(cache);

    if !is_cached {
        _request_badge_to_file(&params.redirect_url, &locked_inner.file_path).await?;
    }
    Ok((is_cached, locked_inner.file_path.clone()))
}

async fn get_cached_badge(params: &Params) -> anyhow::Result<Badge> {
    let cache_result = _get_cached_badge(params).await.map_err(|e| {
        slog::error!(LOG, "error requesting badge {:?}", e);
        e
    });
    let (was_cached, file_path) = match cache_result.ok() {
        Some((was_cached, file_path)) => (was_cached, Some(file_path)),
        None => (false, None),
    };
    Ok(Badge {
        was_cached,
        file_path,
        redirect_url: params.redirect_url.clone(),
    })
}

async fn reset_cached_badge(params: &Params) -> anyhow::Result<()> {
    slog::info!(LOG, "dropping cached badge: {}", params.cache_name);
    let mut guard = CACHE.lock().await;
    guard.remove(&params.cache_name);
    Ok(())
}

async fn get_crate(
    web::Path(name): web::Path<String>,
    request: HttpRequest,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let params = Params::new(&name, Kind::Crate, &request).map_err(|e| {
        slog::error!(LOG, "error parsing crate {}: {:?}", name, e);
        actix_web::error::ErrorBadRequest(format!("invalid badge name: {}", name))
    })?;
    let badge = get_cached_badge(&params).await.map_err(|e| {
        slog::error!(LOG, "error retrieving badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error retrieving badge: {}", name))
    })?;
    let resp = badge.into_response(&request).await.map_err(|e| {
        slog::error!(LOG, "error loading badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error loading badge: {}", name))
    })?;
    Ok(resp)
}

async fn get_badge(
    web::Path(name): web::Path<String>,
    request: HttpRequest,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let params = Params::new(&name, Kind::Badge, &request).map_err(|e| {
        slog::error!(LOG, "error parsing badge {}: {:?}", name, e);
        actix_web::error::ErrorBadRequest(format!("invalid badge name: {}", name))
    })?;
    let badge = get_cached_badge(&params).await.map_err(|e| {
        slog::error!(LOG, "error retrieving badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error retrieving badge: {}", name))
    })?;
    let resp = badge.into_response(&request).await.map_err(|e| {
        slog::error!(LOG, "error loading badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error loading badge: {}", name))
    })?;
    Ok(resp)
}

async fn reset_crate(
    web::Path(name): web::Path<String>,
    request: HttpRequest,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let params = Params::new(&name, Kind::Crate, &request)
        .map_err(|_| actix_web::error::ErrorBadRequest(format!("invalid badge name: {}", name)))?;
    reset_cached_badge(&params).await.map_err(|e| {
        slog::error!(LOG, "error resting badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error resting badge: {}", name))
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": "ok",
    })))
}

async fn reset_badge(
    web::Path(name): web::Path<String>,
    request: web::HttpRequest,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let params = Params::new(&name, Kind::Badge, &request)
        .map_err(|_| actix_web::error::ErrorBadRequest(format!("invalid badge name: {}", name)))?;
    reset_cached_badge(&params).await.map_err(|e| {
        slog::error!(LOG, "error resting badge {}: {:?}", name, e);
        actix_web::error::ErrorInternalServerError(format!("error resting badge: {}", name))
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": "ok",
    })))
}

macro_rules! make_file_serve_fns {
    ($([$name:ident, $path:expr]),* $(,),*) => {
        $(
            async fn $name() -> actix_web::Result<NamedFile> {
                Ok(NamedFile::open($path).map_err(|_| actix_web::error::ErrorInternalServerError("asset not found"))?)
            }
        )*
    };
}

make_file_serve_fns!(
    [favicon, "static/favicon.ico"],
    [robots, "static/robots.txt"],
);

async fn status() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": CONFIG.version,
    })))
}

async fn p404() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::NotFound().body("nothing here"))
}

pub async fn start() -> anyhow::Result<()> {
    CONFIG.ensure_loaded()?;

    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    slog::info!(LOG, "** Listening on {} **", addr);

    HttpServer::new(|| {
        actix_web::rt::spawn(cleanup());
        let tera = Tera::new("templates/**/*.html").expect("unable to compile templates");

        App::new()
            .data(tera)
            .wrap(crate::logger::Logger::new())
            .service(
                web::resource("/")
                    .route(web::get().to(index))
                    .route(web::head().to(|| HttpResponse::Ok().header("x-head", "less").finish())),
            )
            .service(
                web::resource("/crates/v/{name}")
                    .route(web::get().to(get_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/crate/{name}")
                    .route(web::get().to(get_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/badge/{name}")
                    .route(web::get().to(get_badge))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset")
                    .route(web::get().to(reset))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset/crates/v/{name}")
                    .route(web::delete().to(reset_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset/crate/{name}")
                    .route(web::delete().to(reset_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset/badge/{name}")
                    .route(web::delete().to(reset_badge))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            // static files
            .service(Files::new("/static", "static"))
            // status
            .service(web::resource("/status").route(web::get().to(status)))
            // special resources
            .service(web::resource("/favicon.ico").route(web::get().to(favicon)))
            .service(web::resource("/robots.txt").route(web::get().to(robots)))
            // 404s
            .default_service(web::resource("").route(web::get().to(p404)))
    })
    .bind(addr)?
    .run()
    .await?;
    Ok(())
}
