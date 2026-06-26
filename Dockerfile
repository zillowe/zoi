FROM archlinux:latest AS builder

# These are used by build.rs to embed telemetry configuration.
# You can override them during the build process, e.g.:
# docker build --build-arg POSTHOG_API_KEY="your_key"
ARG POSTHOG_API_KEY=""
ARG POSTHOG_API_HOST=""
ARG ZOI_DEFAULT_REGISTRY=""
ARG ZOI_AUTHORITIES_KEY_1=""
ARG ZOI_AUTHORITIES_KEY_2=""
ARG ZOI_ABOUT_PACKAGER_AUTHOR=""
ARG ZOI_ABOUT_PACKAGER_EMAIL=""
ARG ZOI_ABOUT_PACKAGER_HOMEPAGE=""

RUN pacman -Syu --noconfirm --needed base-devel pkgconf openssl git rust

# Set the working directory.
WORKDIR /usr/src/app

RUN { \
    echo "POSTHOG_API_KEY=${POSTHOG_API_KEY}"; \
    echo "POSTHOG_API_HOST=${POSTHOG_API_HOST}"; \
    echo "ZOI_DEFAULT_REGISTRY=${ZOI_DEFAULT_REGISTRY}"; \
    echo "ZOI_AUTHORITIES_KEY_1=${ZOI_AUTHORITIES_KEY_1}"; \
    echo "ZOI_AUTHORITIES_KEY_2=${ZOI_AUTHORITIES_KEY_2}"; \
    echo "ZOI_ABOUT_PACKAGER_AUTHOR=${ZOI_ABOUT_PACKAGER_AUTHOR}"; \
    echo "ZOI_ABOUT_PACKAGER_EMAIL=${ZOI_ABOUT_PACKAGER_EMAIL}"; \
    echo "ZOI_ABOUT_PACKAGER_HOMEPAGE=${ZOI_ABOUT_PACKAGER_HOMEPAGE}"; \
    } > .env

COPY Cargo.toml Cargo.lock ./

COPY crates ./crates

RUN cargo build --bin zoi --release

FROM archlinux:base

RUN pacman -Syu --noconfirm --needed git ca-certificates gnupg bubblewrap && pacman -Scc --noconfirm

COPY --from=builder /usr/src/app/target/release/zoi /usr/local/bin/zoi

RUN groupadd -r zoi && useradd -r -g zoi -s /bin/bash -d /home/zoi zoi && \
    mkdir -p /home/zoi/.zoi && \
    chown -R zoi:zoi /home/zoi

USER zoi

WORKDIR /home/zoi

ENTRYPOINT ["zoi"]

CMD ["--help"]
