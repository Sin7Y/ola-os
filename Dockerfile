FROM rust:1.70 as planner
WORKDIR olaos
# We only pay the installation cost once,
# it will be cached from the second build onwards
# To ensure a reproducible build consider pinning
# the cargo-chef version with `--version X.X.X`
RUN cargo install cargo-chef
COPY . .
# Compute a lock-like file for our project
RUN cargo chef prepare  --recipe-path recipe.json

FROM rust:1.70 as cacher
WORKDIR olaos
RUN cargo install cargo-chef
COPY --from=planner /olaos/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.70 as builder
WORKDIR olaos
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /olaos/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
ENV DATABASE_URL postgres://postgres:password@host.docker.internal:15432/olaos
ENV SQLX_OFFLINE true
# Build our application, leveraging the cached deps!
RUN cargo build --release --bin ola_node

FROM rust:1.70-slim as runtime
WORKDIR olaos
COPY --from=builder /olaos/target/release/ola_node ./ola_node
COPY config/configuration config/configuration
COPY etc etc
ENV OLAOS_APP_ENVIRONMENT production
ENV OLAOS_IN_DOCKER true
ENV OLAOS_DATABASE_POOL_SIZE 50
ENV OLAOS_SEQUENCER_FEE_ACCOUNT_ADDR 0xde03a0B5963f75f1C8485B355fF6D30f3093BDE7
ENV OLAOS_SEQUENCER_ENTRYPOINT_HASH 0xfefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe
ENV OLAOS_SEQUENCER_DEFAULT_AA_HASH 0xfefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe
ENV OLAOS_CHAIN_ETH_NETWORK localhost
ENV OLAOS_CHAIN_ETH_OLA_NETWORK_ID 270
ENV OLAOS_CHAIN_ETH_OLA_NETWORK_NAME localhost
ENTRYPOINT ["./ola_node"]
