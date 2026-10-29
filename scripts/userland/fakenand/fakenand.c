/*
 * fakenand: runs a console NAND tool under qemu-arm against files instead of a console
 * (run.sh). Preloaded into the tool (LD_PRELOAD), it
 *
 * - answers hakchi's NAND ioctls on the kernel's terms (drivers/block/nand/nfd/nand_blk.c):
 *   hakchi_test, phy_read and phy_write against a raw image of the NAND's first blocks
 *   ($FAKENAND_RAW, 0x20000-byte blocks), and BLKGETSIZE, BLKGETSIZE64, BLKRRPART, BLKFLSBUF on
 *   the regular files standing in for /dev/nand*;
 * - logs to $FAKENAND_LOG every file it opens, every ioctl, raw write, mount and program it
 *   starts, and does not start the programs (execv exits 0 in the child) nor mount anything.
 *
 * The device files themselves are qemu's: run.sh gives qemu a root (-L) where /dev/nand* and
 * /proc/... are ordinary files.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

#define BLOCK_SIZE 0x20000u
#define PAGE_SIZE 2048u
#define PAGES_PER_BLOCK 64u
#define BLOCK_COUNT 4096u

enum {
	hakchi_test = _IO('v', 121),
	burn_boot0 = _IO('v', 122),
	burn_boot1 = _IO('v', 123),
	phy_read = _IO('v', 124),
	phy_write = _IO('v', 125),
	read_boot0 = _IO('v', 126),
};

/* The kernel's struct burn_param_t (32-bit ARM). */
struct burn_param {
	void *buffer;
	uint32_t length;
	uint32_t offset;
	uint32_t flags;
	void *oob_buffer;
	uint32_t oob_length;
	uint32_t badblocks;
};

struct hakchi_nandinfo {
	char str[8];
	uint32_t size;
	uint32_t page_size;
	uint32_t pages_per_block;
	uint32_t block_count;
};

static FILE *logf_;

static void say(const char *fmt, ...) {
	va_list ap;
	if (logf_ == NULL) {
		const char *p = getenv("FAKENAND_LOG");
		logf_ = (p != NULL) ? fdopen(((int (*)(const char *, int, ...))dlsym(RTLD_NEXT, "open"))(
					      p, O_WRONLY | O_APPEND | O_CREAT, 0644),
				      "a")
				    : stderr;
		if (logf_ == NULL)
			logf_ = stderr;
		setvbuf(logf_, NULL, _IOLBF, 0);
	}
	fprintf(logf_, "[%d] ", (int)getpid());
	va_start(ap, fmt);
	vfprintf(logf_, fmt, ap);
	va_end(ap);
	fputc('\n', logf_);
}

/* Raw blocks: reads beyond the image are erased NAND (0xff), writes beyond it are refused. */
static int raw_io(int write, struct burn_param *p) {
	const char *path = getenv("FAKENAND_RAW");
	int (*real_open)(const char *, int, ...) = dlsym(RTLD_NEXT, "open");
	int fd;
	off_t size;
	ssize_t n;
	if (path == NULL || p->length % BLOCK_SIZE || p->offset % BLOCK_SIZE) {
		errno = EINVAL;
		return -1;
	}
	fd = real_open(path, write ? O_RDWR : O_RDONLY);
	if (fd < 0)
		return -1;
	size = lseek(fd, 0, SEEK_END);
	if (write) {
		if ((off_t)p->offset + p->length > size) {
			close(fd);
			errno = EIO;
			return -1;
		}
		n = pwrite(fd, p->buffer, p->length, p->offset);
	} else {
		memset(p->buffer, 0xff, p->length);
		n = ((off_t)p->offset < size) ? pread(fd, p->buffer, p->length, p->offset) : 0;
	}
	close(fd);
	if (n < 0)
		return -1;
	p->badblocks = 0;
	p->flags = 0;
	return 0;
}

int ioctl(int fd, unsigned long req, ...) {
	static int (*real)(int, unsigned long, ...);
	va_list ap;
	void *arg;
	struct stat st;
	va_start(ap, req);
	arg = va_arg(ap, void *);
	va_end(ap);
	if (real == NULL)
		real = dlsym(RTLD_NEXT, "ioctl");

	if (req == hakchi_test) {
		struct burn_param *p = arg;
		struct hakchi_nandinfo hni = {"hakchi", sizeof(hni), PAGE_SIZE, PAGES_PER_BLOCK, BLOCK_COUNT};
		memcpy(p->buffer, &hni, sizeof(hni));
		p->flags = 0;
		p->badblocks = 0;
		say("ioctl hakchi_test");
		return 0;
	}
	if (req == phy_read || req == phy_write) {
		struct burn_param *p = arg;
		int w = (req == phy_write);
		say("ioctl %s block 0x%x count %u raw %u", w ? "phy_write" : "phy_read",
		    p->offset / BLOCK_SIZE, p->length / BLOCK_SIZE, p->flags & 3);
		if (w && getenv("FAKENAND_READONLY") != NULL) {
			say("  refused: FAKENAND_READONLY");
			errno = EROFS;
			return -1;
		}
		return raw_io(w, p);
	}
	if (req == burn_boot0 || req == burn_boot1 || req == read_boot0) {
		say("ioctl 0x%lx (boot0/boot1): not emulated", req);
		errno = ENOTTY;
		return -1;
	}
	if (fstat(fd, &st) == 0 && S_ISREG(st.st_mode)) {
		switch (req) {
		case 0x1260: /* BLKGETSIZE */
			*(unsigned long *)arg = (unsigned long)(st.st_size / 512);
			say("ioctl BLKGETSIZE fd %d -> %lu", fd, *(unsigned long *)arg);
			return 0;
		case 0x80041272: /* BLKGETSIZE64 */
			*(uint64_t *)arg = (uint64_t)st.st_size;
			say("ioctl BLKGETSIZE64 fd %d -> %llu", fd, (unsigned long long)st.st_size);
			return 0;
		case 0x125f: /* BLKRRPART */
			say("ioctl BLKRRPART fd %d", fd);
			return 0;
		case 0x1261: /* BLKFLSBUF */
			say("ioctl BLKFLSBUF fd %d", fd);
			return 0;
		}
	}
	say("ioctl 0x%lx fd %d: passed on", req, fd);
	return real(fd, req, arg);
}

int open(const char *path, int flags, ...) {
	static int (*real)(const char *, int, ...);
	va_list ap;
	mode_t mode;
	va_start(ap, flags);
	mode = va_arg(ap, mode_t);
	va_end(ap);
	if (real == NULL)
		real = dlsym(RTLD_NEXT, "open");
	say("open %s %s", path, (flags & O_ACCMODE) == O_RDONLY ? "r" : "rw");
	return real(path, flags, mode);
}

FILE *fopen(const char *path, const char *mode) {
	static FILE *(*real)(const char *, const char *);
	if (real == NULL)
		real = dlsym(RTLD_NEXT, "fopen");
	say("fopen %s %s", path, mode);
	return real(path, mode);
}

/* Programs are logged, not started, except those whose path contains $FAKENAND_EXEC. */
int execv(const char *path, char *const argv[]) {
	static int (*real)(const char *, char *const[]);
	const char *allow = getenv("FAKENAND_EXEC");
	char line[4096];
	size_t n = 0;
	int i;
	line[0] = '\0';
	for (i = 0; argv[i] != NULL && n < sizeof(line) - 2; i++)
		n += (size_t)snprintf(line + n, sizeof(line) - n, "%s%s", i ? " " : "", argv[i]);
	say("exec %s: %s", path, line);
	if (allow != NULL && *allow && strstr(path, allow) != NULL) {
		if (real == NULL)
			real = dlsym(RTLD_NEXT, "execv");
		return real(path, argv);
	}
	_exit(0);
}

/* Commands are logged, not run, except sunxi-nand-part when $FAKENAND_NANDPART names an ARM build
 * of it: that one runs under qemu-arm in the same root, so the partition table it writes lands
 * in the fake /dev/nand. A command containing $FAKENAND_FAIL fails instead, with exit code
 * $FAKENAND_FAIL_CODE (default 1). */
FILE *popen(const char *cmd, const char *mode) {
	static FILE *(*real)(const char *, const char *);
	const char *tool = getenv("FAKENAND_NANDPART");
	const char *root = getenv("FAKENAND_ROOT");
	const char *at = strstr(cmd, "\"sunxi-nand-part\"");
	char run[8192];
	if (real == NULL)
		real = dlsym(RTLD_NEXT, "popen");
	say("popen %s: %s", mode, cmd);
	if (getenv("FAKENAND_FAIL") != NULL && *getenv("FAKENAND_FAIL") && strstr(cmd, getenv("FAKENAND_FAIL"))) {
		char fail[32];
		snprintf(fail, sizeof(fail), "exit %d",
			 getenv("FAKENAND_FAIL_CODE") ? atoi(getenv("FAKENAND_FAIL_CODE")) : 1);
		say("  failing it: FAKENAND_FAIL (%s)", fail);
		return real(fail, mode);
	}
	if (tool != NULL && root != NULL && at != NULL) {
		snprintf(run, sizeof(run), "%.*sqemu-arm -L %s %s%s", (int)(at - cmd), cmd, root, tool,
			 at + strlen("\"sunxi-nand-part\""));
		say("  running: %s", run);
		return real(run, mode);
	}
	return real("true", mode);
}

int mount(const char *src, const char *dst, const char *type, unsigned long flags, const void *data) {
	say("mount %s %s %s 0x%lx %s", src ? src : "-", dst ? dst : "-", type ? type : "-", flags,
	    data ? (const char *)data : "-");
	return 0;
}

int umount(const char *dst) {
	say("umount %s", dst);
	return 0;
}

int umount2(const char *dst, int flags) {
	say("umount2 %s 0x%x", dst, flags);
	return 0;
}

int unshare(int flags) {
	say("unshare 0x%x", flags);
	return 0;
}

void sync(void) {
	say("sync");
}
