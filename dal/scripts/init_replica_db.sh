#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: psql is not installed."
    exit 1
fi

if ! [ -x "$(command -v sqlx)" ]; then
    echo >&2 "Error: sqlx is not installed."
    echo >&2 "Use:"
    echo >&2 " cargo install --version=0.5.7 sqlx-cli --no-default-features --features postgres"
    echo >&2 "to install it."
    exit 1
fi

# Check if a custom user has been set, otherwise default to 'postgres'
DB_USER=${POSTGRES_USER:=admin}
# Check if a custom password has been set, otherwise default to 'password'
DB_PASSWORD="${POSTGRES_PASSWORD:=admin123}"
# Check if a custom database name has been set, otherwise default to 'olaos'
DB_NAME="${POSTGRES_DB:=olaos_replica}"
# Check if a custom port has been set, otherwise default to '5433'
DB_PORT="${POSTGRES_PORT:=5433}"

if [[ -z "${SKIP_DOCKER}" ]]
then
    docker run \
        --mount type=bind,source="$(pwd)"/scripts/replica_postgresql.conf,target=/etc/postgresql/postgresql.conf \
        -v ./scripts/data-backup:/var/lib/postgresql/data \
        --net olaos-db-sync \
        --name olaos_replica \
        -e POSTGRES_USER=${DB_USER} \
        -e POSTGRES_PASSWORD=${DB_PASSWORD} \
        -e POSTGRES_DB=${DB_NAME} \
        -p "${DB_PORT}":5432 \
        -d postgres \
        postgres -c config_file=/etc/postgresql/postgresql.conf
        # default config_file: /var/lib/postgresql/data/postgresql.conf
fi

export PGPASSWORD="${DB_PASSWORD}"
until psql -h "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
    >&2 echo "Postgres is still unavailable - sleeping"
    sleep 1
done

>&2 echo "Postgres is up and running on port ${DB_PORT}!, ready to go!"