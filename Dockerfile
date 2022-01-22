FROM rust:1.58.1-bullseye as builder

# create a new empty shell
RUN USER=root cargo new --bin badge-cache
WORKDIR /badge-cache

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# # this build step will cache your dependencies
RUN cargo build --release

RUN rm ./target/release/badge-cache*
RUN rm ./target/release/deps/badge_cache*
RUN mkdir cache_dir

RUN rm ./src/*.rs

# # copy source
COPY ./src ./src

COPY ./.git .git
RUN git rev-parse HEAD | head -c 7 | awk '{ printf "%s", $0 >"commit_hash.txt" }'
RUN rm -rf .git

# # build for release
RUN cargo build --release

# copy all static files
COPY ./static ./static
COPY ./templates ./templates

RUN mkdir ./bin
RUN cp ./target/release/badge-cache ./bin/badge-cache
RUN rm -rf ./target

FROM debian:bullseye-slim
RUN apt-get update && apt-get install --yes ca-certificates
COPY --from=builder /badge-cache /badge-cache
WORKDIR /badge-cache

# set the startup command to run your binary
CMD ["./bin/badge-cache"]
