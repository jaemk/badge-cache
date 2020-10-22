#![recursion_limit = "1024"]

mod logger;
mod service;

use std::env;
use std::fs;
use std::io::Read;

use slog::{o, Drain};

fn env_or(k: &str, default: &str) -> String {
    env::var(k).unwrap_or_else(|_| default.to_string())
}

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = Config::load();

    // The "base" logger that all crates should branch off of
    pub static ref BASE_LOG: slog::Logger = {
        if CONFIG.log_format == "pretty" {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::CompactFormat::new(decorator).build().fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            let drain = slog::LevelFilter::new(drain, slog::Level::Debug).fuse();
            slog::Logger::root(drain, o!())
        } else {
            let drain = slog_json::Json::default(std::io::stderr()).fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            let drain = slog::LevelFilter::new(drain, slog::Level::Info).fuse();
            slog::Logger::root(drain, o!())
        }
    };

    // Base logger
    pub static ref LOG: slog::Logger = BASE_LOG.new(slog::o!("app" => "badge-cache"));
}

#[derive(serde_derive::Deserialize)]
pub struct Config {
    pub version: String,
    pub host: String,
    pub port: u16,
    pub log_format: String,
    pub max_ext_length: usize,
    pub cache_ttl_millis: u128,
    pub cache_dir: String,
    pub http_expiry_seconds: i64,
    pub default_file_ext: String,
}
impl Config {
    pub fn load() -> Self {
        let version = fs::File::open("commit_hash.txt")
            .map(|mut f| {
                let mut s = String::new();
                f.read_to_string(&mut s).expect("Error reading commit_hasg");
                s
            })
            .unwrap_or_else(|_| "unknown".to_string());
        Self {
            version,
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "4000").parse().expect("invalid port"),
            log_format: env_or("LOG_FORMAT", "json")
                .to_lowercase()
                .trim()
                .to_string(),
            max_ext_length: env_or("MAX_EXT_LENGTH", "1024")
                .parse()
                .expect("invalid max_ext_length"),
            cache_ttl_millis: env_or(
                "CACHE_TTL_MILLIS",
                (60 * 60 * 24 * 1000).to_string().as_str(),
            )
            .parse()
            .expect("invalid cache_ttl_millis"),
            cache_dir: env_or("CACHE_DIR", "cache_dir"),
            http_expiry_seconds: env_or("HTTP_EXPIRY_SECONDS", (60 * 60).to_string().as_str())
                .parse()
                .expect("invalid http_expiry_seconds"),
            default_file_ext: env_or("DEFAULT_FILE_EXT", "svg"),
        }
    }
    pub fn ensure_loaded(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

async fn run() -> anyhow::Result<()> {
    slog::info!(
        LOG, "initializing";
        "version" => &CONFIG.version,
        "host" => &CONFIG.host,
        "port" => &CONFIG.port,
        "log_format" => &CONFIG.log_format,
        "max_ext_length" => &CONFIG.max_ext_length,
        "cache_ttl_millis" => &CONFIG.cache_ttl_millis,
        "cache_dir" => &CONFIG.cache_dir,
        "http_expiry_seconds" => &CONFIG.http_expiry_seconds,
        "default_file_ext" => &CONFIG.default_file_ext,
    );
    service::start().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let local = tokio::task::LocalSet::new();
    let sys = actix_web::rt::System::run_in_tokio("server", &local);
    if let Err(e) = run().await {
        slog::error!(LOG, "Error: {:?}", e);
    }
    if let Err(e) = sys.await {
        slog::error!(LOG, "system failure, Error: {:?}", e);
    }
}
