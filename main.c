#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <sys/mman.h>
#include <sys/stat.h>


#define _stringize2(x) #x
#define _stringize(x) _stringize2(x)
#define _source_loc() __FILE__ ":" _stringize(__LINE__)

#define _local static


#include <stdint.h>
#define i8    int8_t
#define i16   int16_t
#define i32   int32_t
#define i64   int64_t
#define u8    uint8_t
#define u16   uint16_t
#define u32   uint32_t
#define u64   uint64_t


// TODO: regroup in header of constant values / platform values
static const size_t PageSize = 0x1000;


// TODO: eventually move these signatures to header ?
/* check that given boolean is true, otherwise print format string and exit */
void fail_at_location_if(int has_failed, const char *loc, const char * msg, ...);
#define _fail_if(has_failed, msg, args...) fail_at_location_if(has_failed, _source_loc(), msg, args)

/* A file mapped into memory for manipulation in the editor, + its metadata */
struct mapped_file {
	char name[256]; // '\0' terminated filepath
	struct stat file_stat;
	u8* data;
	// TODO: load timestamp
};

/* WRITEME */
int alloc_pages(void *base_addr, int n_page);
/* WRITEME */
int mapped_file_load(struct mapped_file *f);



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

// TODO: R or RW ?
// TODO: SHARED or PRIVATE ? if using MAP_SHARED, I should detect other process writing to the file
int mapped_file_load(struct mapped_file *f)
{
	int fd = open(f->name, O_RDONLY);
	_fail_if(fd < 0, "open %s failed: %s", f->name, strerror(errno));

	int r = fstat(fd, &f->file_stat);
	_fail_if(r < 0, "stat %s failed: %s", f->name, strerror(errno));

	int prot = PROT_READ;
	int flags = MAP_FILE | MAP_SHARED;
	int offset = 0;

	f->data = mmap(NULL, f->file_stat.st_size, prot, flags, fd, offset);
	_fail_if(f->data == MAP_FAILED, "mapped_file_load(%s) failed: %s", f->name, strerror(errno));


	close(fd);
	// TODO: return mapped_file instead
	return 0;
}

int mapped_file_print(int out_fd, struct mapped_file *f)
{
	u8* c = f->data;
	u8* stop = f->data + f->file_stat.st_size;
	while (c < stop) {
		u8* line_end = memchr(c, '\n', (size_t)(stop - c)) + 1;
		write(out_fd, c, (size_t)(line_end - c));
		// TODO: check errors
		c = line_end;
	}
	return 0;
}

int alloc_pages(void *base_addr, int n_page) {
	size_t size = n_page * PageSize;
	int prot = PROT_READ | PROT_WRITE;
	int flags = MAP_PRIVATE | MAP_ANON | MAP_FIXED;
	int offset = 0;
	void* real_addr = mmap(base_addr, size, prot, flags, -1, offset);
	if (real_addr == MAP_FAILED) {
		return -1;
	}
	assert(real_addr == base_addr);
	return  0;
}

int main(int argc, char **args)
{
	puts("hello chizel !!");

	void *addr = (void*) 0xffff000000;
	int r = alloc_pages(addr, 24);
	_fail_if(r < 0, "alloc_pages(mmap) failed: %s", strerror(errno));

	struct mapped_file f = {
		.name =
			"/Users/hugobenichi/Desktop/editor/czl/Makefile",
			//__FILE__,
	};
	mapped_file_load(&f);

	mapped_file_print(STDOUT_FILENO, &f);
}
