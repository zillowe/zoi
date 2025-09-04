FROM rust:1-slim-bookworm AS builder

# These are used by build.rs to embed telemetry configuration.
# You can override them during the build process, e.g.:
# docker build --build-arg POSTHOG_API_KEY="your_key"
ARG POSTHOG_API_KEY="phc_abcdefg"
ARG POSTHOG_API_HOST="https://eu.i.posthog.com"

RUN apt-get update && apt-get install -y build-essential pkg-config libssl-dev git && rm -rf /var/lib/apt/lists/*

# Set the working directory.
WORKDIR /usr/src/app

RUN echo "POSTHOG_API_KEY=${POSTHOG_API_KEY}" > .env
RUN echo "POSTHOG_API_HOST=${POSTHOG_API_HOST}" >> .env

COPY Cargo.toml Cargo.lock ./

COPY .cargo ./.cargo

COPY build.rs ./

COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y git ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/zoi /usr/local/bin/zoi

RUN groupadd -r zoi && useradd -r -g zoi -s /bin/bash -d /home/zoi zoi && \
    mkdir -p /home/zoi/.zoi && \
    chown -R zoi:zoi /home/zoi

USER zoi

WORKDIR /home/zoi

ENTRYPOINT ["zoi"]

CMD ["--help"]
