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


#define _arraylen(ary)    (sizeof(ary)/sizeof(ary[0]))

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

typedef int (*char_filter)(u8);

struct slice {
	u8* start;
	u8* stop;


	int writ_to(int fd)
	{
		return write(fd, (void*) start, (size_t)(stop - start));
	}

	i32 len()
	{
		return stop - start;
	}

	i32 is_empty()
	{
		return stop == start;
	}

	char* str()
	{
		int l = len();
		char* str = (char*) malloc(l + 1);
		memcpy(str, start, l);
		str[l] = 0;
		return str;
	}

	slice drop(i32 n)
	{
		slice s = *this;
		n = _min(n, s.len());
		s.start += n;
		return s;
	}

	slice split(int c)
	{
		slice left = *this;
		u8* pivot = (u8*) memchr(left.start, c, left.len());
		if (!pivot) {
			pivot = stop;
		}

		left.stop = pivot;
		start = pivot;

		return left;
	}

	slice strip(u8 c)
	{
		slice s = *this;
		while (s.len() && *(s.stop - 1) == c) {
			s.stop--;
		}
		return s;
	}

	slice take_line()
	{
		slice line = split('\n');
		if (*start == '\n') {
			start++;
		}
		return line;
	}

	slice slice_while(char_filter fn, int inverse)
	{
		u8* u = start;
		while (u < stop && inverse ^ fn(*u)) {
			u++;
		}
		slice left = *this;
		left.stop = u;
		start = u;
		return left;

	}

	slice take_while(char_filter fn)
	{
		return slice_while(fn, 0);
	}

	slice drop_until(char_filter fn)
	{
		return slice_while(fn, 1);
	}

};

static const slice slice_empty = { .start = 0, .stop = 0 };

int char_is_space(u8 c) {
	return c == ' ' || c == '\t';
}

struct keyval {
	struct slice key;
	struct slice val;
};

struct keyval keyval_from_line(slice s)
{
	struct keyval kv;

	s = s.split('#');

	s.take_while(char_is_space);
	kv.key = s.drop_until(char_is_space);

	s.take_while(char_is_space);
	kv.val = s.drop_until(char_is_space);

	return kv;
}

struct mapped_file {
	char name[256];
	struct stat file_stat;
	struct slice data;
};

int mapped_file_load(struct mapped_file *f, const char* path)
{
	memcpy(&f->name, path, _arraylen(f->name));

	int fd = open(f->name, O_RDONLY);
	_fail_if(fd < 0, "open %s failed: %s", f->name, strerror(errno));

	int r = fstat(fd, &f->file_stat);
	_fail_if(r < 0, "stat %s failed: %s", f->name, strerror(errno));

	int prot = PROT_READ;
	int flags = MAP_SHARED;
	int offset = 0;

	size_t len = f->file_stat.st_size;
	f->data.start = (u8*) mmap(NULL, len, prot, flags, fd, offset);
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
	struct mapped_file config;
	mapped_file_load(&config, "./config.txt");

	while (config.data.len()) {
		slice line = config.data.take_line();
		if (line.is_empty()) {
			continue;
		}

		struct keyval kv = keyval_from_line(line);
		if (kv.key.is_empty()) {
			continue;
		}

		if (kv.val.len()) {
			printf("key:%s val:%s\n", kv.key.str(), kv.val.str());
		} else {
			printf("W: no val for key '%s'\n", kv.key.str());
		}
	}
}
