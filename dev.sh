# TODO: Persist data between restarts
# TODO: Use env for config

# TODO: How do we do DDL changes

# docker run --name=vttestserver --rm \
#   -p 33577:33578 \
#   --health-cmd="mysqladmin ping -h127.0.0.1 -P33577" \
#   --health-interval=5s \
#   --health-timeout=2s \
#   --health-retries=5 \
#   -v vttestserver_data:/vt/vtdataroot \
#   vitess/vttestserver:mysql80 \
#   /vt/bin/vttestserver \
#   --alsologtostderr \
#   --data_dir=/vt/vtdataroot/ \
#   --persistent_mode \
#   --port=33574 \
#   --mysql_bind_host=0.0.0.0 \
#   --keyspaces=test,unsharded \
#   --num_shards=2,1


# git clone https://github.com/vitessio/vitess.git
# cd vitess
# git checkout release-18.0
# make docker_local

docker run --platform linux/amd64 -p 14200:14200 -p 14201:14201 -p 15000:15000 -p 15001:15001 -p 15991:15991 -p 15999:15999 -p 16000:16000 -e VITE_VTADMIN_API_ADDRESS=http://localhost:14200 --rm -it vitess/local