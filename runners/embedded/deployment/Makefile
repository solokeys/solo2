ifneq ($(filter gen_init_fs list_fs fs.hex fs.raw all,$(MAKECMDGOALS)),)
NEEDS_FS_VARS := true
endif
ifeq ($(MAKECMDGOALS),)
NEEDS_FS_VARS := true
endif

ifeq ($(NEEDS_FS_VARS),true)
ifeq ($(FS_BASE),)
$(error FS_BASE variable missing.)
endif
ifeq ($(FS_SIZE),)
$(error FS_SIZE variable missing.)
endif
endif

CFLAGS := -Wall -Wextra -g2 -O0 -I./lfs

CFLAGS_CUSTOM := $(CFLAGS) -DFILESYSTEM_BASE=$(FS_BASE) -DFILESYSTEM_SIZE=$(FS_SIZE)

all: gen_init_fs list_fs fs.hex

clean:
	rm -f fs.hex fs.raw gen_init_fs list_fs lfs/*.o lfs/lfs.a

fs.hex:	fs.raw
	objcopy -I binary -O ihex --adjust-vma $$(( $(FS_BASE) )) $^ $@
	@wc -l $@
	sed -i -e '/^:10....00FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF../ d' -e '/^:......03/ d' $@
	@wc -l $@

fs.raw: gen_init_fs
	rm -f $@
	./$^ $@

gen_init_fs: gen_init_fs.c lfs/lfs.a
	gcc $(CFLAGS_CUSTOM) $^ lfs/lfs.a -o $@

list_fs: list_fs.c lfs/lfs.a
	gcc $(CFLAGS_CUSTOM) $^ lfs/lfs.a -o $@

lfs/lfs.a: lfs/lfs.o lfs/lfs_util.o
	rm -f $@
	ar qcs $@ $^

lfs/lfs.o lfs/lfs_util.o: %.o: %.c
	gcc $(CFLAGS) $^ -c -o $@
