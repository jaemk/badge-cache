# Badge-rs

> Simple shields.io badge cache

## Setup / Usage

> Cache is only valid for the lifetime of the program. Restarting will invalidate any cached badges

> `libssl-dev` is required for fetching from shields.io

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
* `cargo run -- --help`

Note, if you build an artifact, it needs to be run from the project root

