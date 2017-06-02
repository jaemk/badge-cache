# Badge-rs

> Simple `img.shields.io` cache for rust crates and generic badges

`badge-cache` supports the same url api as `img.shields.io` while enforcing a `Cache-Control: max-age=3600` (1 hr) client-side cache. Badges generated from `img.shields.io` are cached server-side for 12 hrs.

## Setup / Usage

> `libssl-dev` is required for `reqwest` for fetching from shields.io

* Run a dev instance `cargo run -- serve` -> `localhost:3000`
* Expose a direct instance to the world `cargo build --release`, `sudo target/release/badge-cache serve --public --port 80`
* Setup behind a proxy
    * setup ssl certs (if you want), see `letsencrypt.info`
    * copy `nginx.conf.sample` to `/etc/nginx/sites-available/badge` and update with your project/site info
        * ! don't turn on the secure redirect or the `Strict-Transport-Security max-age` unless you already have your cert
        * `sudo nginx -t`
        * `sudo systemctl restart nginx`
    * copy `badge.service.sample` to `/etc/systemd/system/badge.service` and update with your project info
        * `sudo systemctl enable badge`
        * `sudo systemctl start badge`
        * check the logs `sudo journalctl -f -u badge`
* Clearing the cache:
    * The server will do a sweep of its cache every hour to clear out expired items.
    * Cached files can be forcefully deleted using the `admin` helper:
        * `target/release/badge-cache admin --clear-cached-badges /<PATH_TO_PROJ>/static/badges`
            * live dangerously: `--no-confirm`
        * Setup a cron job to forcefully delete all cached files every other day:
            * `0 0 2-30/2 * * /<PATH_TO_PROJ>/target/release/badge-cache admin --clear-cached-badges /<PATH_TO_PROJ>/static/badges --no-confirm >> /var/log/badge.log 2>&1`
* `cargo run -- --help`

Note, if you build an artifact, it needs to be run from the project root so it can find its `static/badges` directory

