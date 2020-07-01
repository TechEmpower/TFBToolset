FROM rust:latest

WORKDIR /tfb-toolset

RUN apt install -y openssl libssl-dev

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml

# This is to download, compile, and cache dependencies before adding real src
RUN mkdir src
RUN touch src/lib.rs
RUN cargo build --release
RUN rm src/lib.rs

COPY src src

CMD cargo build --release
