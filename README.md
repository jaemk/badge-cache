# Badge-Cache

> Simple `img.shields.io` cache for rust crates and generic badges

`badge-cache` supports the same url api as `img.shields.io` while enforcing a `Cache-Control: max-age=3600` (1 hr) client-side cache. Badges generated from `img.shields.io` are cached server-side for 12 hrs unless explicitly reset.

## Running

```
cargo run
# or
./docker.sh run
```

## Options and defaults

```
# Environment vars
# The following can be set to override the defaults listed here:

# host to listen on
HOST=0.0.0.0

# port to listen on
PORT=3003

# how to format logs, 'json' for programmatic consumption
# or 'pretty' for human consumption
LOG_FORMAT=json

# log level filter
LOG_LEVEL=INFO

# max badge name length before truncating
MAX_NAME_LENGTH=512

# max badge ext length before truncating
MAX_EXT_LENGTH=512

# max badge query string length before truncating
MAX_QS_LENGTH=512

# ttl on cached badges
CACHE_TTL_MILLIS=86400000

# relative directory where cached badges should be stored
CACHE_DIR=cache_dir

# cache-control expiry to set on http responses
HTTP_EXPIRY_SECONDS=3600

# default badge file type if not specified
DEFAULT_FILE_EXT=svg

# initial delay before wiping badges on startup
CLEANUP_DELAY_SECONDS=5

# interval between cache sweeps
CLEANUP_INTERVAL_SECONDS=300
```

