FROM rust:1.35

# create a new empty shell
RUN USER=root cargo new --bin badge-cache
WORKDIR /badge-cache

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# # this build step will cache your dependencies
# RUN cargo build --release
# RUN rm src/*.rs

# # copy all source/static/resource files
COPY ./src ./src
COPY ./static ./static
COPY ./templates ./templates

# # build for release
# RUN rm ./target/release/deps/badge_cache*
# RUN cargo build --release

### builds are broken
RUN curl -L https://github.com/jaemk/badge-cache/releases/download/v0.2.1/badge-cache-v0.2.1-x86_64-unknown-linux-musl.tar.gz -o _badge.tar.gz
RUN tar -xf _badge.tar.gz

# set the startup command to run your binary
CMD ["./badge-cache", "serve", "--port", "80", "--public"]
