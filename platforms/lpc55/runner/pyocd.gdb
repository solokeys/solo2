set history save on
set confirm off

target remote localhost:3333
monitor arm semihosting enable
# load
monitor reset halt
# continue
