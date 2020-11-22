set history save on
set confirm off
target remote :1234
# target remote | qemu-system-arm -cpu cortex-m33 -machine mps2-an505 -nographic -gdb stdio -S -kernel empty.elf

# set print asm-demangle on
# monitor arm semihosting enable

load
