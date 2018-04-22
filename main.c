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
#define _global static


// Standard int types
#include <stdint.h>
typedef int8_t    i8;
typedef int16_t   i16;
typedef int32_t   i32;
typedef int64_t   i64;
typedef uint8_t   u8;
typedef uint16_t  u16;
typedef uint32_t  u32;
typedef uint64_t  u64;


// geom primitive types
struct vec {
	i32 x;
	i32 y;
};

struct rec {
	i32 x;                    // min x
	i32 y;                    // min y
	i32 w;                    // max x - min x
	i32 h;                    // max y - min y
};

typedef struct vec vec;
typedef struct rec rec;

inline vec v(i32 x ,i32 y)
{
	return (vec){
		.x = x,
		.y = y,
	};
}

inline rec r(vec min, vec max)
{
	assert(min.x <= max.x);
	assert(min.y <= max.y);
	return (rec){
		.x = min.x,
		.y = min.y,
		.w = max.x - min.x,
		.h = max.y - min.y,
	};
}

inline vec rec_min(rec r)
{
	return v(r.x, r.y);
}

inline vec rec_max(rec r)
{
	return v(r.w - r.x, r.h - r.y);
}

inline vec rec_diag(rec r)
{
	return v(r.w, r.h);
}


// geom operators
inline vec add_vec_vec(vec v0, vec v1)
{
	return v(v0.x + v1.x, v0.y + v1.y);
}

inline rec add_vec_rec(vec v0, rec r1)
{
	return r(add_vec_vec(v0, rec_min(r1)), add_vec_vec(v0, rec_max(r1)));
}

inline rec add_rec_vec(rec r0, vec v1)
{
	return add_vec_rec(v1, r0);
}

inline vec sub_vec_vec(vec v0, vec v1)
{
	return v(v0.x - v1.x, v0.y - v1.y);
}

inline rec sub_rec_vec(rec r0, vec v1)
{
	return r(sub_vec_vec(rec_min(r0), v1), sub_vec_vec(rec_max(r0), v1));
}

#define add(a,b) _Generic((a),          \
	rec:	add_rec_vec,            \
	vec:	add_vec_other(b))((a),(b))

#define add_vec_other(b) _Generic((b),  \
	rec:	add_vec_rec,            \
	vec:	add_vec_vec)

#define sub(a,b) _Generic((a),          \
	rec:	sub_rec_vec,            \
	vec:	sub_vec_vec)((a),(b))




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
		u8* line_end = memchr(c, '\n', (size_t)(stop - c));
		line_end++;
		write(out_fd, c, (size_t)(line_end - c));
		// TODO: check errors
		c = line_end;
	}
	return 0;
}

int alloc_pages(void *base_addr, int n_page)
{
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
	int z = alloc_pages(addr, 24);
	_fail_if(z < 0, "alloc_pages(mmap) failed: %s", strerror(errno));

	struct mapped_file f = {
		.name =
			"/Users/hugobenichi/Desktop/editor/czl/Makefile",
			//__FILE__,
	};
	mapped_file_load(&f);

	mapped_file_print(STDOUT_FILENO, &f);



	rec r0 = r(v(1, 4), v(8, 9));
	vec v1 = v(5, 6);
	vec v2 = v(3, 2);

	//rec r1 = add(r0, v1);
	//rec r2 = add(v2, r0);

	vec v3 = add(v1, v2);

	vec v4 = sub(v3, v2);
	//rec r3 = sub(r2, v3);
}
