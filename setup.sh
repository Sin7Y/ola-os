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
