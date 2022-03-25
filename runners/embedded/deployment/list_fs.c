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
	// printf("F RD %02x+%04x %04x (%p)\n", b, o, sz, __builtin_return_address(1));
	(void)c;
	memcpy(buf, ((char *)fsmmap) + (b<<12) + o, sz);
	return LFS_ERR_OK;
}

static int mm_prog(const struct lfs_config *c, lfs_block_t b, lfs_off_t o, const void *buf, lfs_size_t sz) {
	// printf("F WR %02x+%04x %04x (%p)\n", b, o, sz, __builtin_return_address(1));
	(void)c;
	memcpy(((char *)fsmmap) + (b<<12) + o, buf, sz);
	return LFS_ERR_OK;
}

static int mm_erase(const struct lfs_config *c, lfs_block_t b) {
	// printf("F ER %02x\n", b);
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

static void recurse(int depth, char *dn) {
	int r;
	struct lfs_dir ldir;
	struct lfs_info linfo;

	memset(&ldir, 0, sizeof(ldir));

	r = lfs_dir_open(&LFS, &ldir, dn);
	if (r != LFS_ERR_OK) { printf("dirO %s %d\n", dn, r); exit(1); }

	while (1) {
		memset(&linfo, 0, sizeof(linfo));
		r = lfs_dir_read(&LFS, &ldir, &linfo);
		if (r == 0) break;
		if (r < 0) { printf("dirR %s %d\n", dn, r); exit(1); }

		if ((linfo.type == LFS_TYPE_DIR) && (!strcmp(linfo.name, ".") || !strcmp(linfo.name, ".."))) {
			continue;
		}

		printf("%.*s+ %c %06x %s\n", depth*2, "         ", linfo.type == LFS_TYPE_REG ? 'f' : 'd', linfo.type == LFS_TYPE_REG ? linfo.size : 0, linfo.name);

		if (linfo.type == LFS_TYPE_DIR) {
			char next_dn[LFS_NAME_MAX+1];
			snprintf(next_dn, LFS_NAME_MAX, "%s/%s", dn, linfo.name);
			recurse(depth+1, next_dn);
		}
	}

	r = lfs_dir_close(&LFS, &ldir);
	if (r != LFS_ERR_OK) { printf("dirC %s %d\n", dn, r); exit(1); }
}
	
int main(int an, char **ac) {
	int r;

	int f = open(ac[1], O_RDONLY);
	fsmmap = mmap((void *)FILESYSTEM_BASE, FILESYSTEM_SIZE, PROT_READ, MAP_FIXED | MAP_SHARED, f, 0);
	if (fsmmap == MAP_FAILED) return 1;

	r = lfs_mount(&LFS, &LFS_CONFIG);
	if (r != LFS_ERR_OK) { printf("mount %d\n", r); exit(1); }

	recurse(0, "/");

	r = lfs_unmount(&LFS);
	if (r != LFS_ERR_OK) { printf("unmount %d\n", r); exit(1); }

	return 0;
}
