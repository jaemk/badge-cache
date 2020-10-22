use actix_files::{Files, NamedFile};
use actix_web::HttpResponse;
use actix_web::HttpServer;
use actix_web::{web, App};
use async_mutex::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tera::{Context, Tera};

use crate::{CONFIG, LOG};

#[derive(Debug, Clone)]
pub struct CachedFile {
    cache_name: String,
    created_millis: u128,
    file_path: PathBuf,
}

lazy_static::lazy_static! {
    pub static ref CACHE: Mutex<HashMap<String, CachedFile>> = {
        Mutex::new(HashMap::with_capacity(512))
    };
}

async fn index(
    template: web::Data<tera::Tera>,
) -> actix_web::Result<HttpResponse, actix_web::Error> {
    let s = template
        .render("landing.html", &Context::new())
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
    fn new(full_name: &str, kind: Kind, request: &web::HttpRequest) -> anyhow::Result<Params> {
        let parts = full_name.split('.').collect::<Vec<_>>();
        let (name, ext) = if parts.len() < 2 {
            (full_name.to_string(), CONFIG.default_file_ext.clone())
        } else {
            let name = parts[0].to_string();
            let ext = parts.into_iter().skip(1).collect::<String>();
            let ext = if ext.len() > CONFIG.max_ext_length {
                let (ext_head, _) = ext.split_at(CONFIG.max_ext_length);
                ext_head.to_string()
            } else {
                ext
            };
            (name, ext)
        };

        let query_params = request.query_string().to_string();
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
    file_path: Option<std::path::PathBuf>,
    redirect_url: String,
}
impl Badge {
    async fn into_response(self, request: &web::HttpRequest) -> anyhow::Result<HttpResponse> {
        if let Some(p) = self.file_path {
            let mut resp = NamedFile::open(p)?
                .into_response(request)
                .map_err(|e| anyhow::anyhow!("asset not found: {:?}", e))?;
            let hdrs = resp.headers_mut();

            let ctrl = actix_web::http::HeaderValue::from_str(&format!(
                "max-age={}, public",
                CONFIG.http_expiry_seconds
            ))?;
            hdrs.insert(actix_web::http::header::CACHE_CONTROL, ctrl);

            let expiry_dt = chrono::Utc::now()
                .checked_add_signed(chrono::Duration::seconds(CONFIG.http_expiry_seconds))
                .ok_or_else(|| anyhow::anyhow!("error creating expiry datetime"))?;
            let exp = actix_web::http::HeaderValue::from_str(&expiry_dt.to_rfc2822())?;
            hdrs.insert(actix_web::http::header::EXPIRES, exp);
            hdrs.insert(
                actix_web::http::HeaderName::from_static("x-was-cached"),
                actix_web::http::HeaderValue::from_str(&format!("{}", self.was_cached))?,
            );
            Ok(resp)
        } else {
            Ok(HttpResponse::TemporaryRedirect()
                .set_header("Location", self.redirect_url)
                .finish())
        }
    }
}

async fn _request_badge_to_file(params: &Params) -> anyhow::Result<PathBuf> {
    let resp = reqwest::get(&params.redirect_url)
        .await
        .map_err(|e| anyhow::anyhow!("request failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("request read failed: {}", e))?;

    let new_file_path = Path::new(&CONFIG.cache_dir).join(&params.cache_name);
    let new_file_path = web::block(move || -> anyhow::Result<PathBuf> {
        use std::io::Write;
        let mut f = std::fs::File::create(&new_file_path)
            .map_err(|e| anyhow::anyhow!("failed to create file {}", e))?;
        f.write_all(&resp)
            .map_err(|e| anyhow::anyhow!("failed writing response to file {}", e))?;
        Ok(new_file_path)
    })
    .await?;

    Ok(new_file_path)
}

fn now_millis() -> u128 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|dur| dur.as_millis())
        .unwrap_or(0)
}

async fn _get_cached_badge(params: &Params) -> anyhow::Result<(bool, PathBuf)> {
    let cached = {
        let guard = CACHE.lock().await;
        guard.get(&params.cache_name).and_then(|cached_file| {
            let now = now_millis();
            let diff = now - cached_file.created_millis;
            if diff > CONFIG.cache_ttl_millis {
                slog::info!(
                    LOG, "cached file expired";
                    "file" => &params.cache_name,
                );
                None
            } else {
                slog::info!(
                    LOG, "using cached file";
                    "file" => &params.cache_name,
                );
                Some(cached_file.file_path.to_owned())
            }
        })
    };
    let (was_cached, file) = match cached {
        Some(f) => (true, f),
        None => {
            slog::info!(
                LOG, "fetching new content";
                "file" => &params.cache_name,
            );
            let new_file = _request_badge_to_file(params).await?;
            let mut guard = CACHE.lock().await;
            guard.insert(
                params.cache_name.clone(),
                CachedFile {
                    cache_name: params.cache_name.clone(),
                    created_millis: now_millis(),
                    file_path: new_file.clone(),
                },
            );
            (false, new_file)
        }
    };
    Ok((was_cached, file))
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
    request: web::HttpRequest,
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
    request: web::HttpRequest,
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
    request: web::HttpRequest,
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
    let start = actix_web::rt::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut interval =
        actix_web::rt::time::interval_at(start, std::time::Duration::from_secs(60 * 5));
    loop {
        interval.tick().await;
        slog::info!(LOG, "cleaning stale items");

        let now = now_millis();
        let removed_from_cache = {
            let mut guard = CACHE.lock().await;
            let mut removed = vec![];
            guard.retain(|_, v| {
                let diff_ms = now - v.created_millis;
                if diff_ms > CONFIG.cache_ttl_millis {
                    slog::info!(LOG, "invalidating cached item: {}", v.cache_name);
                    removed.push(v.clone());
                    false
                } else {
                    true
                }
            });
            removed
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
                web::resource("/reset/crates/v/{name}")
                    .route(web::get().to(reset_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset/crate/{name}")
                    .route(web::get().to(reset_crate))
                    .route(web::head().to(|| HttpResponse::Ok().finish())),
            )
            .service(
                web::resource("/reset/badge/{name}")
                    .route(web::get().to(reset_badge))
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
