#include <assert.h>
#include <errno.h>
#include <math.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <termios.h>
#include <unistd.h>

#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>




typedef int8_t    i8;
typedef int16_t   i16;
typedef int32_t   i32;
typedef int64_t   i64;
typedef uint8_t   u8;
typedef uint16_t  u16;
typedef uint32_t  u32;
typedef uint64_t  u64;


#define _stringize2(x) #x
#define _stringize(x) _stringize2(x)
#define _source_loc() __FILE__ ":" _stringize(__LINE__)

#define _min(x,y) ((x) < (y) ? (x) : (y))

void fail_at_location_if(int has_failed, const char *loc, const char * msg, ...);
#define _fail_if(has_failed, msg, args...) fail_at_location_if(has_failed, _source_loc(), msg, args)

void fail_at_location_if(int has_failed, const char *loc, const char * msg, ...)
{
	if (!has_failed) {
		return;
	}
	fprintf(stderr, "%s[ ", loc);
        va_list args;
        va_start(args, msg);
        vfprintf(stderr, msg, args);
        va_end(args);
        fprintf(stderr, "\n");
        exit(1);
}

struct slice {
	u8* start;
	u8* stop;
};

struct slice_pair {
	struct slice left;
	struct slice right;
};

int slice_write(int fd, struct slice s)
{
	return write(fd, (void*) s.start, (size_t)(s.stop - s.start));
}

i32 slice_len(struct slice s)
{
	return s.stop - s.start;
}

struct slice slice_drop(struct slice s, i32 n)
{
	n = _min(n, slice_len(s));
	s.start += n;
	return s;
}

struct slice_pair slice_split_at(struct slice s, int c)
{
	u8* pivot = memchr(s.start, c, slice_len(s));
	struct slice_pair p = {
		.left = s,
		.right = s,
	};
	p.left.stop = pivot;
	p.right.start = pivot;
	return p;
}

struct slice slice_take_line(struct slice *s)
{
	u8* pivot = memchr(s->start, '\n', slice_len(*s));
	if (!pivot) {
		pivot = s->start;
	} else {
		pivot++;
	}
	struct slice line = { .start = s->start, .stop = pivot };
	printf("left len:%d\n", slice_len(line));
	s->start = pivot;
	return line;
}

struct mapped_file {
	char name[256];
	struct stat file_stat;
	struct slice data;
};

int mapped_file_load(struct mapped_file *f)
{
	int fd = open(f->name, O_RDONLY);
	_fail_if(fd < 0, "open %s failed: %s", f->name, strerror(errno));

	int r = fstat(fd, &f->file_stat);
	_fail_if(r < 0, "stat %s failed: %s", f->name, strerror(errno));

	int prot = PROT_READ;
	int flags = MAP_FILE | MAP_SHARED;
	int offset = 0;

	size_t len = f->file_stat.st_size;
	f->data.start = mmap(NULL, len, prot, flags, fd, offset);
	f->data.stop  = f->data.start + len;
	_fail_if(f->data.start == MAP_FAILED, "mapped_file_load(%s) failed: %s", f->name, strerror(errno));

	close(fd);
	return 0;
}

struct configuration {
	int argument_a;
	int argument_b;
};

int main(int argc, char** args)
{
	puts("hw");

	struct mapped_file config = {
		.name = "./config.txt"
	};
	mapped_file_load(&config);

	while (slice_len(config.data)) {
		struct slice line = slice_take_line(&config.data);
		slice_write(STDOUT_FILENO, line);
	}
}
