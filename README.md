# Badge-Cache [![Build Status](https://travis-ci.org/jaemk/badge-cache.svg?branch=master)](https://travis-ci.org/jaemk/badge-cache)

Moved to [jaemk/badge](https://github.com/jaemk/badge)


> Simple `img.shields.io` cache for rust crates and generic badges

`badge-cache` supports the same url api as `img.shields.io` while enforcing a `Cache-Control: max-age=3600` (1 hr) client-side cache. Badges generated from `img.shields.io` are cached server-side for 12 hrs unless explicitly reset.

## Setup / Usage

* Download the latest release: `./update.py run`
* Expose a direct instance to the world `bin/badge-cache serve --public --port 80 --log`
* Setup behind a proxy
    * setup ssl certs (if you want), see `letsencrypt.info`
    * copy `nginx.conf.sample` to `/etc/nginx/sites-available/badge` and update with your project/site info
        * `sudo nginx -t`
        * `sudo systemctl restart nginx`
    * copy `badge.service.sample` to `/lib/systemd/system/badge.service` and update with your project info
        * `sudo systemctl enable badge`
        * `sudo systemctl start badge`
        * check the logs `sudo journalctl -f -u badge`
* Clearing the cache:
    * The server will do a sweep of its cache every hour to clear out expired items.
    * Cached files can be forcefully deleted using the `admin` helper:
        * `target/release/badge-cache admin --clear-cached-badges /<PATH_TO_PROJ>/static/badges`
        * Setup a cron job to forcefully delete all cached files every other day:
            * `0 0 2-30/2 * * /<PATH_TO_PROJ>/target/release/badge-cache admin --clear-cached-badges /<PATH_TO_PROJ>/static/badges --no-confirm >> /var/log/badge.log 2>&1`

## Development

> `libssl-dev` is required on linux for `reqwest`

* Run a dev instance `cargo run -- serve --log` -> `localhost:3000`

