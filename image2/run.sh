#!/bin/sh

echo "Starting up Cityscale..."

# echo "Starting components:"
# echo -n "  vtctld"

# vtctld \
#  $TOPOLOGY_FLAGS \
#  --cell $cell \
#  --service_map 'grpc-vtctl,grpc-vtctld' \
#  --backup_storage_implementation file \
#  --file_backup_storage_root $VTDATAROOT/backups \
#  --log_dir $VTDATAROOT/tmp \
#  --port $vtctld_web_port \
#  --grpc_port $grpc_port \
#  --pid_file $VTDATAROOT/tmp/vtctld.pid \
#   > $VTDATAROOT/tmp/vtctld.out 2>&1 &

# echo " - ok"

# ls
cat ./topology

ls /vt/bin

vtcombo -logtostderr=true -proto_topo "$(cat ./topology)" -schema_dir ./vschema -mysql_server_port 15306 -mysql_server_bind_address 0.0.0.0 -mysql_auth_server_impl none -db_socket /tmp/mysql.sock -db_host localhost -mycnf-file /etc/mysql/my.cnf -db_app_user root -db_allprivs_user root -db_appdebug_user root -db_dba_user root -db_repl_user root -dbddl_plugin vttest -port 15000 -grpc_port 15001 -service_map 'grpc-vtgateservice,grpc-vtctl,grpc-vtctld' -db_charset utf8mb4 -vschema_ddl_authorized_users='%'


# TODO