# Badge-rs

> Simple shields.io badge cache

## Setup / Usage

> `libssl-dev` is required for `reqwest` for fetching from shields.io

* Run a dev instance `cargo run -- serve` -> `localhost:3000`
* Expose a direct instance to the world `cargo build --release`, `sudo target/release/badge-cache serve --port 80`
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
    * Use the `admin` helper to delete cached files
        * `target/release/badge-cache admin --clear-cached-badges <path-to-proj-root>/static/badges`
            * live dangerously: `--no-confirm`
        * setup in a cron job, 11am & 11pm
            * `0 11,23 * * * /<PATH_TO_PROJ>/target/release/badge-cache admin --clear-cached-badges /<PATH_TO_PROJ>/static/badges --no-confirm >> /var/log/badge.log 2>&1`
* `cargo run -- --help`

Note, if you build an artifact, it needs to be run from the project root so it can find its `static/badges` directory

