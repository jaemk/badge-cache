FROM rust:1.47

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

# # copy all source/static/resource files
COPY ./src ./src
COPY ./static ./static
COPY ./templates ./templates

# # build for release
RUN cargo build --release

COPY ./.git .git
RUN git rev-parse HEAD | head -c 7 | awk '{ printf "%s", $0 >"commit_hash.txt" }'
RUN rm -rf .git

# set the startup command to run your binary
CMD ["./target/release/badge-cache"]
