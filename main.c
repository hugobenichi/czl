#include <assert.h>
#include <errno.h>
#include <math.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <termios.h>
#include <unistd.h>

#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>


#define _stringize2(x) #x
#define _stringize(x) _stringize2(x)
#define _source_loc() __FILE__ ":" _stringize(__LINE__)

#define _local static
#define _global static


static const char logs_path[] = "/tmp/czl.log";
static FILE *logs;


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

// Careful: the result expr gets evaluated twice
#define _max(x,y) ((x) > (y) ? : (x) : (y))
#define _min(x,y) ((x) < (y) ? : (x) : (y))

// geom primitive types
struct vec {
	i32 x;
	i32 y;
};
typedef struct vec vec;

struct rec {
	union {
		struct {
			vec min;
			vec max;
		};
		struct {
			i32 x0;
			i32 y0;
			i32 x1;
			i32 y1;
		};
	};
};
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
		.min = min,
		.max = max,
	};
}


// geom operators
inline vec add_vec_vec(vec v0, vec v1)
{
	return v(v0.x + v1.x, v0.y + v1.y);
}

inline rec add_vec_rec(vec v0, rec r1)
{
	return r(add_vec_vec(v0, r1.min), add_vec_vec(v0, r1.max));
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
	return r(sub_vec_vec(r0.min, v1), sub_vec_vec(r0.max, v1));
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


inline vec rec_diag(rec r)
{
	return sub_vec_vec(r.max, r.min);
}

inline i32 rec_w(rec r)
{
	return r.x1 - r.x0;
}

inline i32 rec_h(rec r)
{
	return r.y1 - r.y0;
}


char* vec_print(char *dst, size_t len, vec v)
{
	int n = snprintf(dst, len, "{.x=%d, .y=%d}", v.x, v.y);
	return dst + n; //_min(n, len);
}

char* rec_print(char *dst, size_t len, rec r)
{
	i32 w = rec_w(r);
	i32 h = rec_h(r);
	int n = snprintf(dst, len, "{.x0=%d, .y0=%d, .x1=%d, .y1=%d, w=%d, .h=%d}", r.x0, r.y0, r.x1, r.y1, w, h);
	return dst + n; //_min(n, len);
}

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







/* TERMINAL STUFF */

//static const char term_seq_finish[]                       = "\x1b[0m";
//static const char term_clear[]                            = "\x1bc";
//static const char term_newline[]                          = "\r\n";
//static const char term_cursor_hide[]                      = "\x1b[?25l";
//static const char term_cursor_show[]                      = "\x1b[?25h";
static const char term_cursor_save[]                      = "\x1b[s";
static const char term_cursor_restore[]                   = "\x1b[u";
static const char term_switch_offscreen[]                 = "\x1b[?47h";
static const char term_switch_mainscreen[]                = "\x1b[?47l";
static const char term_switch_mouse_event_on[]            = "\x1b[?1000h";
static const char term_switch_mouse_tracking_on[]         = "\x1b[?1002h";
static const char term_switch_mouse_tracking_off[]        = "\x1b[?1002l";
static const char term_switch_mouse_event_off[]           = "\x1b[?1000l";
//static const char term_switch_focus_event_on[]            = "\x1b[?1004h";
//static const char term_switch_focus_event_off[]           = "\x1b[?1004l";

vec term_get_size()
{
	struct winsize w = {};
	int z = ioctl(1, TIOCGWINSZ, &w);
	if (z < 0) {
		perror("window_get_size: ioctl() failed");
		exit(1);
	}
	return v(w.ws_row, w.ws_col);
}

struct termios termios_initial = {};

void term_restore()
{
	tcsetattr(STDIN_FILENO, TCSAFLUSH, &termios_initial);
	//fprintf(stdout, term_switch_focus_event_off);
	fprintf(stdout, term_switch_mouse_tracking_off);
	fprintf(stdout, term_switch_mouse_event_off);
	fprintf(stdout, term_switch_mainscreen);
	fprintf(stdout, term_cursor_restore);
}

void term_raw()
{
	int z;
	z = tcgetattr(STDIN_FILENO, &termios_initial);
	if (z < 0) {
		errno = ENOTTY;
		perror("term_raw: tcgetattr() failed");
		exit(1);
	}
	atexit(term_restore);

	fprintf(stdout, term_cursor_save);
	fprintf(stdout, term_switch_offscreen);
	fprintf(stdout, term_switch_mouse_event_on);
	fprintf(stdout, term_switch_mouse_tracking_on);
	//fprintf(stdout, term_switch_focus_event_on); // TODO: turn on and check if files need reload.

	struct termios termios_raw = termios_initial;
	termios_raw.c_iflag &= ~BRKINT;                    // no break
	termios_raw.c_iflag &= ~ICRNL;                     // no CR to NL
	termios_raw.c_iflag &= ~INPCK;                     // no parity check
	termios_raw.c_iflag &= ~ISTRIP;                    // no strip character
	termios_raw.c_iflag &= ~IXON;                      // no CR to NL
	termios_raw.c_oflag &= ~OPOST;                     // no post processing
	termios_raw.c_lflag &= ~ECHO;                      // no echo
	termios_raw.c_lflag &= ~ICANON;
	termios_raw.c_lflag &= ~ISIG;
	termios_raw.c_cc[VMIN] = 0;                        // return each byte, or nothing when timeout
	termios_raw.c_cc[VTIME] = 100;                     // 100 * 100 ms timeout
	termios_raw.c_cflag |= CS8;                        // 8 bits chars

	z = tcsetattr(STDIN_FILENO, TCSAFLUSH, &termios_raw);
	if (z < 0) {
		errno = ENOTTY;
		perror("term_raw: tcsetattr() failed");
		exit(1);
	}
}







/* FILEBUFFER and TEXT MANAGEMENT */

// A block of memory with a moving cursor. Invariant: start <= cursor <= cursor.
struct buffer {
	u8 *start;
	u8 *stop;
	u8 *cursor;
};

struct buffer buffer_mk(void *ptr, size_t len)
{
	u8* mem = ptr;
	return (struct buffer) {
		.start   = mem,
		.stop    = mem + len,
		.cursor  = mem,
	};
}

void buffer_reset(struct buffer *b)
{
	b->cursor = b->start;
}

struct buffer buffer_sub(struct buffer *b, i32 offset, i32 len)
{
	assert(b->start + offset < b->stop);
	assert(b->start + offset + len < b->stop);
	return buffer_mk(b->start + offset, len);
}

void buffer_append(struct buffer *b, u8 *c, i32 n)
{
	assert(b->cursor + n < b->stop);
	memcpy(b->cursor, c, n);
	b->cursor += n;
}

void buffer_append1(struct buffer *b, u8 c)
{
	buffer_append(b, &c, 1);
}


// Delimits a range of memory. Invariant: start <= stop.
struct slice {
	u8 *start;     // inclusive
	u8 *stop;      // exclusive
};
typedef struct slice slice;

int slice_write(int fd, slice s)
{
	return write(fd, (void*) s.start, (size_t)(s.stop - s.start));
}

// A slice segment iterator that scans a piece of memory slice per slice on every
// instance of 'sep' (interpreted as a char).
struct slice_iter {
	u8* cursor;
	u8* stop;
	i32 sep;
};

i32 slice_iter_next(struct slice_iter *it, slice *s)
{
	u8 *cursor = it->cursor;
	u8 *stop   = it->stop;
	if (stop <= cursor) {
		return 0;
	}
	size_t n_max = (size_t)(cursor - stop);
	u8* line_end = memchr(it->cursor, '\n', n_max);
	line_end++;
	s->start = cursor;
	s->stop  = line_end;
	it->cursor = line_end;
	return 1;
}

struct slice_iter mapped_file_line_iter(struct mapped_file *f)
{
	return (struct slice_iter) {
		.cursor = f->data,
		.stop   = f->data + f->file_stat.st_size,
		.sep    = '\n',
	};
}

void mapped_file_print(int out_fd, struct mapped_file *f)
{
	struct slice_iter it = mapped_file_line_iter(f);
	slice s;
	while (slice_iter_next(&it, &s)) {
		slice_write(out_fd, s);
	}
}

// TODO: helper fns for 1) appending char, 2) wrapping line of text based on separator char


// A buffer of lines represented as slices.
struct line_buffer {
	slice *lines;
	slice *next;
	i32 n_lines;
};

struct line_buffer line_buffer_mk(void* ptr, size_t len)
{
	return (struct line_buffer) {
		.lines   = ptr,
		.next    = ptr,
		.n_lines = sizeof(slice) / len, // remainder of ptr are wasted
	};
};

slice* line_buffer_assign(struct line_buffer *b)
{
	assert(b->next < b->lines + b->n_lines);
	return b->next++;
}

slice *line_buffer_last(struct line_buffer *b)
{
	assert(b->lines < b->next);
	return b->next - 1;
}

i32 line_buffer_used(struct line_buffer *b)
{
	return (i32)(b->next - b->lines);
}


// A block of lines, chained with its neighboring blocks.
// Lines are slices specified as a ptr + len relative to a line buffer.
// Therefore a block always refers to a contiguous array of lines.
struct block {
	struct block *prev;     // previous block
	struct block *next;     // next block
	slice *lines;           // first line of the block
	i32 n_lines;            // number of lines in that block
};

// TODO: decide how the first and last blocks are represented.

// A buffer of blocks contiguous in memory.
struct block_buffer {
	struct block *blocks;
	struct block *next;
	i32 n_blocks;
};

struct block_buffer block_buffer_mk(void* ptr, size_t len)
{
	return (struct block_buffer) {
		.blocks    = ptr,
		.next      = ptr,
		.n_blocks  = sizeof(struct block_buffer) / len, // remainder of ptr are wasted
	};
}

struct block *block_buffer_last(struct block_buffer *b)
{
	assert(b->blocks < b->next);
	return b->next - 1;
}

struct block *block_buffer_assign(struct block_buffer *b)
{
	assert(b->next < b->blocks + b->n_blocks);
	return b->next++;
}

// A type of operation on a block buffer.
enum block_op_type {
	BLOCK_INSERT,
	BLOCK_DELETE,
};

// Edit operation on a chain of blocks.
// For inserts, one block is swapped out and three blocks are swapped in.
// For deletes, one block is swapped out and two blocks are swapped in.
struct block_op {
	enum block_op_type t;
	struct block *new_blocks;
	struct block *old_block;
};

// A buffer of edit operations.
struct block_op_history {
	struct block_op *ops;
	struct block_op *cursor;
	struct block_op *last_op;
	i32 n_ops_max;
};

struct block_op_history block_op_history_mk(void* ptr, size_t len)
{
	return (struct block_op_history) {
		.ops	   = ptr,
		.n_ops_max = sizeof(struct block_op) / len,
		.cursor    = 0,
		.last_op   = 0,
	};
}

struct block_op block_buffer_delete(struct block_buffer *b, struct block *del_block)
{
	struct block *new_upper_block = block_buffer_assign(b);
	struct block *new_lower_block = block_buffer_assign(b);

	new_upper_block->prev = del_block->prev;
	new_upper_block->next = new_lower_block;

	new_lower_block->prev = new_upper_block;
	new_lower_block->next = del_block->next;

	return (struct block_op) {
		.t = BLOCK_DELETE,
		.new_blocks = new_upper_block,
		.old_block  = del_block,
	};
}

// All data relative to a text file representation in memory and insert/delete ops for undo/redo.
struct filebuffer {
	struct mapped_file         f;    // Mapped file, read only
	struct buffer	           a;    // Buffer for insertions, append only
	struct line_buffer         l;    // Buffer for line slices pointing into 'f' or 'a'
	struct block_buffer        b;    // Buffer for blocks of lines
	struct block_op_history	   h;    // Buffer for history of block operations

	struct block *b_first;
	struct block *b_last;
};
typedef struct filebuffer filebuffer;

// TODO: allocator
// TODO: helper fns for
//         1) inserting a block (requires new block pointer, return an insert op)
//         2) deleting a range of blocks (requires two new block pointers, return a delete op)
//         3) appending chars
//         4) locating blocks by line number


// Map a file in memory, scan content for finding all lines, prepare initial line block.
void filebuffer_load(filebuffer *fb)
{
	int z;
	z = mapped_file_load(&fb->f);
	assert(z == 0);

	struct slice_iter it = mapped_file_line_iter(&fb->f);
	slice s;
	while (slice_iter_next(&it, &s)) {
		// CLEANUP: this requires two assignement into an intermediary slice, which is a bit of a pity
		// instead we should be able to write something like:
		//	while (slice_iter_next(&it)) {
		//		slice_iter_get(line_buffer_assign(fb->l);
		//	}
		// but this requires adding a field to the slice_iter
		*line_buffer_assign(&fb->l) = s;
	}

	// Link initial blocks together
	fb->b_first = block_buffer_assign(&fb->b);
	fb->b_last = block_buffer_assign(&fb->b);
	struct block *b = block_buffer_assign(&fb->b);
	fb->b_first->next = b;
	fb->b_last->prev = b;
	b->prev = fb->b_first;
	b->next = fb->b_last;

	// Assign initial block of lines
	b->lines = fb->l.lines;
	b->n_lines = line_buffer_used(&fb->l);
}
/*
    TODOs:
	blit a section to screen (i.e line range iterator)
	delete a range
	insert a range
	undo/redo
	save content
 */




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
	ESC_Z,                      // shift + tab -> "\x1b[Z"
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







#define print_sizeof(str_name) 	printf("sizeof(%s)= %lu\n", #str_name, sizeof(str_name))

int main(int argc, char **args)
{
	logs = fopen(logs_path, "w");
	assert(logs > 0);

	printf("logs address: %p\n", &logs);

	print_sizeof(struct filebuffer);

	puts("hello chizel !!");
	fputs("hello chizel !!\n", logs);

	void *addr = (void*) 0xffff000000;
	int z = alloc_pages(addr, 262144*32);
	// TODO: test claiming for lots of memory with unspecified base address
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

	rec r1 = add(r0, v1);
	rec r2 = add(v2, r1);
	vec v3 = add(v1, v2);

	vec v4 = sub(v3, v2);
	rec r3 = sub(r2, v3);

	char buf[1024] = {};

	*vec_print(buf, 1024, v1) = 0;     puts(buf);
	*vec_print(buf, 1024, v2) = 0;     puts(buf);
	*vec_print(buf, 1024, v3) = 0;     puts(buf);
	*vec_print(buf, 1024, v4) = 0;     puts(buf);

	*rec_print(buf, 1024, r0) = 0;     puts(buf);
	*rec_print(buf, 1024, r1) = 0;     puts(buf);
	*rec_print(buf, 1024, r2) = 0;     puts(buf);
	*rec_print(buf, 1024, r3) = 0;     puts(buf);


//	for (;;) {
//		char* end = key_print(buf, 1024, read_input());
//		*end = 0;
//		puts(buf);
//	}



	term_raw();
	struct key_input k = read_input();

	snprintf(buf, 1024, "read_input: %d\n", k.c);
	fputs(buf, logs);
	puts(buf);

	if (0) {
	slice s = {
		.start = (u8*) buf,
		.stop  = (u8*) buf + 1024,
	};
	slice input = input_capture(s);
	write(STDIN_FILENO, (char*) input.start, input.stop - input.start);
	puts("");
	}

	fflush(logs);
	fclose(logs);
}
