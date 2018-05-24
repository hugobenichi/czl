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


#define DEBUG 0 //1


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

static const struct slice slice_empty = { .start = 0, .stop = 0 };

int slice_write(int fd, struct slice s)
{
	return write(fd, (void*) s.start, (size_t)(s.stop - s.start));
}

i32 slice_len(struct slice s)
{
	return s.stop - s.start;
}

i32 slice_is_empty(struct slice s)
{
	return s.stop == s.start;
}

char* slice_to_string(struct slice s)
{
	int len = slice_len(s);
	char* str = malloc(len + 1);
	memcpy(str, s.start, len);
	str[len] = 0;
	return str;
}

struct slice slice_drop(struct slice s, i32 n)
{
	n = _min(n, slice_len(s));
	s.start += n;
	return s;
}

struct slice slice_split(struct slice *s, int c)
{
	struct slice left = *s;
	u8* pivot = memchr(left.start, c, slice_len(left));
	if (!pivot) {
		pivot = s->stop;
	}

	left.stop = pivot;
	s->start = pivot;

	return left;
}

struct slice slice_strip(struct slice s, u8 c)
{
	while (slice_len(s) && *(s.stop-1) == c) {
		s.stop--;
	}
	return s;
}

struct slice slice_take_line(struct slice *s)
{
	struct slice line = slice_split(s, '\n');
	if (*s->start == '\n') {
		s->start++;
	}
	return line;
}

typedef int (*char_filter)(u8);

struct slice slice_while(struct slice *s, char_filter fn, int inverse)
{
	u8* u = s->start;
	while (u < s->stop && inverse ^ fn(*u)) {
		u++;
	}
	struct slice left = { .start = s->start, .stop = u };
	s->start = u;
	return left;

}

struct slice slice_take_while(struct slice *s, char_filter fn)
{
	return slice_while(s, fn, 0);
}

struct slice slice_drop_until(struct slice *s, char_filter fn)
{
	return slice_while(s, fn, 1);
}

int char_is_space(u8 c) {
	return c == ' ' || c == '\t';
}

struct keyval {
	struct slice key;
	struct slice val;
};

struct keyval keyval_from_line(struct slice s)
{
	struct keyval kv;

	s = slice_split(&s, '#');

	slice_take_while(&s, char_is_space);
	kv.key = slice_drop_until(&s, char_is_space);

	slice_take_while(&s, char_is_space);
	kv.val = slice_drop_until(&s, char_is_space);

	return kv;
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
	int flags = MAP_SHARED;
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
	struct mapped_file config = {
		.name = "./config.txt"
	};
	mapped_file_load(&config);
	int n = 0;

	while (slice_len(config.data)) {
		struct slice line = slice_take_line(&config.data);
		if (slice_is_empty(line)) {
			continue;
		}

		struct keyval kv = keyval_from_line(line);
		if (slice_is_empty(kv.key)) {
			continue;
		}

		if (slice_is_empty(kv.val)) {
			printf("W: no val for key '%s'\n", slice_to_string(kv.key));
		} else {
			printf("key:%s val:%s\n", slice_to_string(kv.key), slice_to_string(kv.val));
		}
	}
}
