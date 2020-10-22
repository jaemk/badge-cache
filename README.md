# Badge-Cache

> Simple `img.shields.io` cache for rust crates and generic badges

`badge-cache` supports the same url api as `img.shields.io` while enforcing a `Cache-Control: max-age=3600` (1 hr) client-side cache. Badges generated from `img.shields.io` are cached server-side for 12 hrs unless explicitly reset.

## Running

```
cargo run
# or
./docker.sh run
```

