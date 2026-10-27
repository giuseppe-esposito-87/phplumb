# syntax=docker/dockerfile:1
# Builds the static Linux x64 phplumb binary. `npm run package -- linux-x64`
# runs it with `--platform linux/amd64` and exports the `artifact` stage.

# The toolchain matches the image, so rust-toolchain.toml triggers no download.
ARG RUST_IMAGE=rust:1.99.1-alpine3.24@sha256:b1d5dd5a22a09404d3e798c0cf051b689077360374eb505fa5bb93c479693d87
ARG RUST_TOOLCHAIN=1.99.1

FROM ${RUST_IMAGE} AS build
ARG TARGETPLATFORM
ARG RUST_TOOLCHAIN
RUN case "$TARGETPLATFORM" in \
      linux/amd64) ;; \
      *) echo "unsupported platform $TARGETPLATFORM: only linux/amd64 is packaged" >&2; exit 1 ;; \
    esac
ENV RUSTUP_TOOLCHAIN=${RUST_TOOLCHAIN}
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN --mount=type=cache,id=phplumb-cargo-registry-linux-amd64,target=/usr/local/cargo/registry \
    --mount=type=cache,id=phplumb-target-linux-amd64,target=/src/target \
    cargo build --release --locked -p phplumb-server \
    && cp target/release/phplumb /phplumb

FROM scratch AS artifact
COPY --from=build /phplumb /phplumb
