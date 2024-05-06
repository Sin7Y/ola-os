#!/bin/bash

# Container names
containers=("olaos_main" "olaos_replica")

# Iterate over the containers
for container_name_or_id in "${containers[@]}"; do
    # Check if the container exists
    if docker inspect "$container_name_or_id" >/dev/null 2>&1; then
        # If the container exists, stop it
        echo "Container $container_name_or_id exists, stopping the container..."
        if docker stop "$container_name_or_id" >/dev/null 2>&1; then
            echo "Container $container_name_or_id stopped successfully."
            sleep 1
            # Remove the container
            echo "Removing container $container_name_or_id..."
            if docker rm "$container_name_or_id" >/dev/null 2>&1; then
                echo "Container $container_name_or_id removed successfully."
            else
                echo "Failed to remove container $container_name_or_id."
            fi
        else
            echo "Failed to stop container $container_name_or_id."
        fi
    else
        # If the container does not exist, print a message
        echo "Container $container_name_or_id does not exist, no action needed."
    fi
done

# remove rocks db
rm -rf ./db &&

# remove object store files
rm -rf ./artifacts &&

# remove pg db and log
cd dal &&
rm -rf ./scripts/archivelog &&
rm -rf ./scripts/data-backup &&
rm -rf ./scripts/olaos_pgdata &&

# start new pg
./scripts/init_main_db.sh &&
./scripts/init_replica_db.sh &&

cd ..
