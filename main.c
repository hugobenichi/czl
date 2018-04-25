#include <assert.h>
#include <errno.h>
#include <math.h>
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

// Careful: the result gets evaluated twice
#define max(x,y) ((x) > (y) ? : (x) : (y))
#define min(x,y) ((x) < (y) ? : (x) : (y))

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

struct slice {
	u8 *start;
	u8 *stop;
};
typedef struct slice slice;

struct block {
	i32 prev;	     // index of previous
	i32 next;            // and next block in BLOCK_BUFFER
	i32 n_newline;       // number of newline chars in that block
	slice text;
};

#define BLOCK_BUFFER (128 * 1024)
_global struct block blocks[BLOCK_BUFFER] = {};            // = 128k blocks = 3.5MB
_global int block_next_index = 0;


/* Blocks
 *   - stored in a static fixed-sized array,
 *   - referenced by their indexes into that static array,
 *   - contains references to previous and next block
 *   - contains pointers into text data (either mapped file or the append buffer)
 *   - a file is a chain of blocks
 *
 *   - reqs:  insert block, delete, split block
 */


#define APPEND_BUFFER_SIZE (1024 * 1024)
_global u8 append_buffer[APPEND_BUFFER_SIZE] = {};         // = 1MB
_global u8* append_cursor = append_buffer;
_global const u8* append_cursor_end = append_buffer + APPEND_BUFFER_SIZE;

void append_char(u8 c) {
	assert(append_cursor < append_cursor_end);
	*append_cursor++ = c;
}

void append_input(u8 *input, size_t len) {
	assert(append_cursor + len < append_cursor_end);
	memcpy(append_cursor, input, len);
	append_cursor += len;
	// TODO: return struct block
}


/* KEY INPUT HANDLING */

enum key_code {
	NO_KEY           = 0,
	CTRL_C           = 3,
	CTRL_D           = 4,
	CTRL_F           = 6,
	CTRL_H           = 8,
	TAB              = 9,       // also ctrl + i
	RETURN           = 10,      // also ctrl + j
	CTRL_K           = 11,
	CTRL_L           = 12,
	ENTER            = 13,
	CTRL_Q           = 17,
	CTRL_S           = 19,
	CTRL_U           = 21,
	CTRL_Z           = 26,
	ESC              = 27,      // also ctrl + [
	BACKSPACE        = 127,

	SOFTCODE_BASE    = 128,     // non-ascii escape sequences and other events
	UNKNOWN_ESC_SEQ,
	ESC_Z,                      // shift + tab -> "\027[Z"
	RESIZE,
	CLICK,
	CLICK_RELEASE,
	ERROR,
	KEY_CODE_END,
};

struct key_input {
	i32 c;
	struct {
		i16 x;
		i16 y;
	} click;
	// TODO: add errno ?
};

_global const char* key_code_names[KEY_CODE_END] = {
	[NO_KEY]                = "NO_KEY",
	[CTRL_C]                = "CTRL_C",
	[CTRL_D]                = "CTRL_D",
	[CTRL_F]                = "CTRL_F",
	[CTRL_H]                = "CTRL_H",
	[TAB]                   = "TAB",
	[RETURN]                = "RETURN",
	[CTRL_K]                = "CTRL_K",
	[CTRL_L]                = "CTRL_L",
	[ENTER]                 = "ENTER",
	[CTRL_Q]                = "CTRL_Q",
	[CTRL_S]                = "CTRL_S",
	[CTRL_U]                = "CTRL_U",
	[CTRL_Z]                = "CTRL_Z",
	[ESC]                   = "ESC",
	[BACKSPACE]             = "BACKSPACE",
	[UNKNOWN_ESC_SEQ]       = "UNKNOWN_ESC_SEQ",
	[ESC_Z]                 = "ESC_Z",
	[ERROR]                 = "ERROR",
	[RESIZE]                = "RESIZE",
	[CLICK]                 = "CLICK",
	[CLICK_RELEASE]         = "CLICK_RELEASE",
	//default]	       "UNKNOWN",
};

i32 is_printable(i32 c)
{
	return ESC < c && c < BACKSPACE;
}

char* key_print(char *dst, size_t len, struct key_input k)
{
	if (is_printable(k.c)) {
		*dst = k.c;
		return dst + 1;
	}

	switch (k.c) {
	case NO_KEY:
	case CTRL_C:
	case CTRL_D:
	case CTRL_F:
	case CTRL_H:
	case TAB:
	case CTRL_K:
	case CTRL_L:
	case ENTER:
	case CTRL_Q:
	case CTRL_S:
	case CTRL_U:
	case CTRL_Z:
	case ESC:
	case BACKSPACE:
	case SOFTCODE_BASE:
	case UNKNOWN_ESC_SEQ:
	case ESC_Z:
	case RESIZE:
	case CLICK:
	case CLICK_RELEASE:
	case ERROR:             return stpncpy(dst, key_code_names[k.c], len);
	default:                return dst + snprintf(dst, len, "UNKNOWN(%d)", k.c);
	}
}

_local struct key_input k(i32 k)
{
	return (struct key_input) {
		.c = k,
	};
}

_local struct key_input k_click(i32 k, i32 x, i32 y)
{
	return (struct key_input) {
		.c = k,
		.click.x = x,
		.click.y = y,
	};
}

_local i32 read_char()
{
	u8 c;

	int n = 0;
	while (n == 0) {
		n = read(STDIN_FILENO, &c, 1);
	}

	// Terminal resize events send SIGWINCH signals which interrupt read()
	if (n < 0 && errno == EINTR) {
		return RESIZE;
	}
	// TODO: error should not be fatal ?
	if (n < 0) {
		perror("read_input failed");
		return ERROR;
	}

	return c;
}

struct key_input read_input()
{
	i32 c = read_char();

	if (c != ESC) {
		return k(c);
	}

	// TODO: support unicode !

	// Escape sequence
	assert(read_char() == '[');
	c = read_char();
	switch (c) {
	case 'M':              break; // Mouse click, handled separately
	case 'Z':              return k(ESC_Z);
	default:               return k(UNKNOWN_ESC_SEQ);
	}

	// Mouse click
	// TODO: support other mouse modes
	c = read_char();
	i32 x = read_char() - 33;
	i32 y = read_char() - 33;
	if (x < 0) {
		x += 255;
	}
	if (y < 0) {
		y += 255;
	}

	switch (c) {
	case 0:
	case 1:
	case 2:                return k_click(CLICK, x, y);
	case 3:                return k_click(CLICK_RELEASE, x, y);
	default:               return k(UNKNOWN_ESC_SEQ);
	}
}

slice input_capture(slice buffer)
{
	slice cursor = {
		.start = buffer.start,
		.stop  = buffer.start,
	};
	while (cursor.stop < buffer.stop) {
		struct key_input k = read_input();
		if (is_printable(k.c)) {
			*cursor.stop++ = k.c;
		}
		if (k.c == TAB) {
			*cursor.stop++ = '\t';
		}
		// TODO: support UTF8
		if (k.c == RETURN) {
			break;
		}
		// else, skip
	}
	return cursor;
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


	char buf[1024] = {};
//	for (;;) {
//		char* end = key_print(buf, 1024, read_input());
//		*end = 0;
//		puts(buf);
//	}

	slice s = {
		.start = (u8*) buf,
		.stop  = (u8*) buf + 1024,
	};
	slice input = input_capture(s);
	write(STDIN_FILENO, (char*) input.start, input.stop - input.start);
	puts("");
}
