#include <stdio.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/mman.h>
#include <fcntl.h>
#include "lfs.h"

#if !defined(FILESYSTEM_BASE) || !defined(FILESYSTEM_SIZE)
# error Definitions for FILESYSTEM_BASE and FILESYSTEM_SIZE missing.
#endif

static void *fsmmap = NULL;

static int mm_read(const struct lfs_config *c, lfs_block_t b, lfs_off_t o, void *buf, lfs_size_t sz) {
	printf("F RD %02x+%04x %04x (%p)\n", b, o, sz, __builtin_return_address(1));
	(void)c;
	memcpy(buf, ((char *)fsmmap) + (b<<12) + o, sz);
	return LFS_ERR_OK;
}

static int mm_prog(const struct lfs_config *c, lfs_block_t b, lfs_off_t o, const void *buf, lfs_size_t sz) {
	printf("F WR %02x+%04x %04x (%p)\n", b, o, sz, __builtin_return_address(1));
	(void)c;
	memcpy(((char *)fsmmap) + (b<<12) + o, buf, sz);
	return LFS_ERR_OK;
}

static int mm_erase(const struct lfs_config *c, lfs_block_t b) {
	printf("F ER %02x\n", b);
	memset(((char *)fsmmap) + (b<<12), 0xff, c->block_size);
	return LFS_ERR_OK;
}

static int mm_noop(const struct lfs_config *c) {
	(void)c;
	return LFS_ERR_OK;
}

static struct lfs LFS;
static struct lfs_config LFS_CONFIG = {
	.context = NULL,
	.read = mm_read,
	.prog = mm_prog,
	.erase = mm_erase,
	.sync = mm_noop,
#ifdef LFS_THREADSAFE
	.lock = mm_noop,
	.unlock = mm_noop,
#endif
	.read_size = 4,
	.prog_size = 4,
	.block_size = 0x1000,
	.block_count = FILESYSTEM_SIZE / 0x1000,
	.block_cycles = -1,
	.cache_size = 256,
	.lookahead_size = 32,
	.read_buffer = NULL,
	.prog_buffer = NULL,
	.lookahead_buffer = NULL,
	.name_max = 0,
	.file_max = 0,
	.attr_max = 0,
	.metadata_max = 0
};
static struct lfs_file LFS_FILE;

int main(int an, char **ac) {
	int r;

	if (an != 2) {
		fprintf(stderr, "Error: output file name missing\n");
		return 1;
	}

	fsmmap = mmap((void *)FILESYSTEM_BASE, FILESYSTEM_SIZE, PROT_READ | PROT_WRITE, MAP_FIXED | MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	if (fsmmap == MAP_FAILED) return 1;

	memset(fsmmap, 0xff, FILESYSTEM_SIZE);

	r = lfs_format(&LFS, &LFS_CONFIG);
	printf("format done, ret: %d\n", r);

	r = lfs_mount(&LFS, &LFS_CONFIG);
	printf("mount done, ret: %d\n", r);

	r = lfs_mkdir(&LFS, "/fido");
	printf("mkdir /fido done, ret: %d\n", r);
	r = lfs_mkdir(&LFS, "/fido/x5c");
	printf("mkdir /fido/x5c done, ret: %d\n", r);
	r = lfs_mkdir(&LFS, "/fido/sec");
	printf("mkdir /fido/sec done, ret: %d\n", r);

	memset(&LFS_FILE, 0, sizeof(LFS_FILE));
	r = lfs_file_open(&LFS, &LFS_FILE, "/fido/x5c/00", LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC);
	printf("open /fido/x5c/00 done, ret: %d\n", r);
	if (r == 0) {
		char buffer[4096];
		int r2 = open("./fido.crt", O_RDONLY);
		int len = read(r2, buffer, 4096);
		close(r2);

		r = lfs_file_write(&LFS, &LFS_FILE, buffer, len);
		printf("file write done, ret: %d\n", r);
		r = lfs_file_close(&LFS, &LFS_FILE);
		printf("file close done, ret: %d\n", r);
	}

	memset(&LFS_FILE, 0, sizeof(LFS_FILE));
	r = lfs_file_open(&LFS, &LFS_FILE, "/fido/sec/00", LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC);
	printf("open /fido/sec/00 done, ret: %d\n", r);
	if (r == 0) {
		char buffer[4096];
		int r2 = open("./fido.key", O_RDONLY);
		int len = read(r2, buffer, 4096);
		close(r2);

		r = lfs_file_write(&LFS, &LFS_FILE, buffer, len);
		printf("file write done, ret: %d\n", r);
		r = lfs_file_close(&LFS, &LFS_FILE);
		printf("file close done, ret: %d\n", r);
	}

	r = lfs_unmount(&LFS);
	printf("unmount done, ret: %d\n", r);

	r = open(ac[1], O_WRONLY | O_CREAT | O_EXCL, 0644);
	write(r, fsmmap, FILESYSTEM_SIZE);
	close(r);
	printf("file written to fd %d\n", r);

	return 0;
}
