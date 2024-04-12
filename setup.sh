# remove db and pg log
rm -rf ./db &&
cd dal &&
rm -rf ./scripts/archivelog ./scripts/data-backup ./scripts/olsoa_pgdata &&

# start new pg
./scripts/init_main_db.sh &&
./scripts/init_replica_db.sh &&

cd ..
