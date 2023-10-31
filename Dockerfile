FROM php:8.1-apache as apache

# FROM buildpack-deps:buster as rust

RUN apt-get update && apt-get install -y wget git

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    SCCACHE_DIR=/tmp/sccache \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=nightly-2022-11-18 \
    SCCACHE_VERSION=v0.2.15 \
    SCCACHE_CACHE_SIZE=1G \
    RUSTUP_TOOLCHAIN=nightly-2022-11-18

ENV RUSTFLAGS="-Awarnings -L ${RUSTUP_HOME}/toolchains/${RUST_VERSION}-x86_64-unknown-linux-gnu/lib" \
    LD_LIBRARY_PATH="${RUSTUP_HOME}/toolchains/${RUST_VERSION}-x86_64-unknown-linux-gnu/lib"

# Install Rust
RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) rustArch='x86_64-unknown-linux-gnu'; rustupSha256='0b2f6c8f85a3d02fde2efc0ced4657869d73fccfce59defb4e8d29233116e6db' ;; \
        armhf) rustArch='armv7-unknown-linux-gnueabihf'; rustupSha256='f21c44b01678c645d8fbba1e55e4180a01ac5af2d38bcbd14aa665e0d96ed69a' ;; \
        arm64) rustArch='aarch64-unknown-linux-gnu'; rustupSha256='673e336c81c65e6b16dcdede33f4cc9ed0f08bde1dbe7a935f113605292dc800' ;; \
        i386) rustArch='i686-unknown-linux-gnu'; rustupSha256='e7b0f47557c1afcd86939b118cbcf7fb95a5d1d917bdd355157b63ca00fc4333' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    url="https://static.rust-lang.org/rustup/archive/1.26.0/${rustArch}/rustup-init"; \
    wget "$url"; \
    echo "${rustupSha256} *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup component add rustc-dev; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# Install sccache
RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) sccacheArch='x86_64'; sccacheSha256='e5d03a9aa3b9fac7e490391bbe22d4f42c840d31ef9eaf127a03101930cbb7ca' ;; \
        arm64) sccacheArch='aarch64'; sccacheSha256='90d91d21a767e3f558196dbd52395f6475c08de5c4951a4c8049575fa6894489' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    dirname="sccache-${SCCACHE_VERSION}-${sccacheArch}-unknown-linux-musl"; \
    filename="${dirname}.tar.gz"; \
    url="https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/${filename}"; \
    wget "$url"; \
    echo "${sccacheSha256} *${filename}" | sha256sum -c -; \
    tar -xvzf ${filename}; \
    mv ${dirname}/sccache /usr/local/bin/sccache; \
    chmod +x /usr/local/bin/sccache; \
    mkdir -p $SCCACHE_DIR; \
    chmod -R a+x $SCCACHE_DIR; \
    rm -rf ${filename} ${dirname};

# Install Yuga
COPY rust-toolchain.toml /tmp/rust-toolchain.toml
# COPY crawl /tmp/crawl
# RUN set -eux; \
#    cargo install --locked --path /tmp/crawl --bin yuga-runner --bin unsafe-counter; \
#    rm -rf /tmp/rust-toolchain.toml /tmp/crawl;

COPY . /tmp/yuga/
RUN set -eux; \
    cd /tmp/yuga; \
    ./install-release.sh; \
    rm -rf /tmp/yuga/;

RUN chmod -R a+w $CARGO_HOME;

COPY web/ /var/www/html/
RUN chmod +x /var/www/html/run-yuga.sh

COPY run-apache2.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/run-apache2.sh
CMD [ "run-apache2.sh" ]
