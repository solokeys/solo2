# source .gdb-dashboard

set history save on
set confirm off

target extended-remote :2331
monitor reset
quit
