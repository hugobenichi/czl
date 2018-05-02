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







/* BASE MEMORY UTILS */

#define print_sizeof(str_name) 	printf("sizeof(%s)= %luB\n", #str_name, sizeof(str_name))

#define Kilo(x) (1024L * (x))
#define Mega(x) (1024L * Kilo(x))
#define Giga(x) (1024L * Mega(x))


// TODO: regroup in header of constant values / platform values
static const size_t PageSize = 0x1000;

// TODO: eventually move these signatures to header ?
/* check that given boolean is true, otherwise print format string and exit */
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

void* v_alloc(u64 base_addr_lit, size_t size)
{
	void *base_addr = (void*) base_addr_lit;
	int prot = PROT_READ | PROT_WRITE;
	int flags = MAP_PRIVATE | MAP_ANON | MAP_FIXED;
	int offset = 0;
	void* real_addr = mmap(base_addr, size, prot, flags, -1, offset);
	if (real_addr == MAP_FAILED) {
		return NULL;
	}
	assert(real_addr == base_addr);
	return  base_addr;
}

typedef void *(*allocator)(size_t);
//static allocator generic_alloc = malloc;




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

// A chunk of memory with a moving cursor. Invariant: start <= cursor <= cursor.
struct buffer {
	u8 *start;
	u8 *stop;
	u8 *cursor;
};
typedef struct buffer buffer;

buffer buffer_mk(void *ptr, size_t len)
{
	u8* mem = ptr;
	return (buffer) {
		.start   = mem,
		.stop    = mem + len,
		.cursor  = mem,
	};
}

void buffer_reset(buffer *b)
{
	b->cursor = b->start;
}

buffer buffer_sub(buffer *b, i32 offset, i32 len)
{
	assert(b->start + offset < b->stop);
	assert(b->start + offset + len < b->stop);
	return buffer_mk(b->start + offset, len);
}

void buffer_append(buffer *b, u8 *c, i32 n)
{
	assert(b->cursor + n < b->stop);
	memcpy(b->cursor, c, n);
	b->cursor += n;
}

void buffer_append1(buffer *b, u8 c)
{
	buffer_append(b, &c, 1);
}

struct buffer_index {
	i32 offset;
};
typedef struct buffer_index buffer_index;

static const buffer_index buffer_index_invalid = {
	.offset = -1,
};

inline buffer_index buffer_index_mk(i32 o) {
	assert(0 <= o);
	return (buffer_index) {
		.offset = 0,
	};
}

inline buffer_index buffer_index_offset(buffer_index i, i32 o)
{
	return buffer_index_mk(i.offset + o);
}

inline void* buffer_index_to_ptr(buffer *b, buffer_index i)
{
	return (void*) (b->start + i.offset);
}

inline buffer_index buffer_ptr_to_index(buffer *b, void* ptr)
{
	u8* addr = ptr;
	assert(b->start <= addr);
	assert(addr < b->stop);
	return buffer_index_mk((i32) (addr - b->start));
}

inline buffer_index buffer_get_top_index(buffer *b)
{
	return buffer_ptr_to_index(b, b->cursor);
}

inline buffer_index buffer_alloc(buffer *b, size_t len)
{
	assert(b->cursor + len <= b->stop);
	buffer_index idx = buffer_get_top_index(b);
	b->cursor += len;
	return idx;
}

#define _def_buffer_getter(name, ptr_t)                                         \
	inline ptr_t buffer_get_ ## name (buffer *b_ptr, buffer_index b_idx)    \
	{                                                                       \
		return (ptr_t) buffer_index_to_ptr(b_ptr, b_idx);               \
	}

#define _def_buffer_alloc(name, t)                                              \
	inline buffer_index buffer_alloc_ ## name (buffer *b_ptr) {             \
		return buffer_alloc(b_ptr, sizeof(t));                                 \
	}


struct slice;
struct block;
struct block_op;

_def_buffer_getter(line, struct slice*);
_def_buffer_getter(block, struct block*);
_def_buffer_getter(block_op, struct block_op*);

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

inline i32 slice_len(slice s) {
	return s.stop - s.start;
}

inline i32 slice_is_empty(slice s) {
	return s.stop <= s.start;
}

slice slice_split(slice *s, i32 sep)
{
	slice front = {};
	slice back = *s;
	i32 len = slice_len(back);
	if (!len) {
		return front;
	}
	front.start = back.start;
	front.stop = memchr(back.start, sep, len);
	front.stop++;
	s->start = front.stop;
	return front;
}

// TODO: slice helper fns for 1) appending char, 2) subslicing


// A block of lines, chained with its neighboring blocks.
// Lines are slices specified as a ptr + len relative to a line buffer.
// Therefore a block always refers to a contiguous array of lines.
struct block {
	buffer_index prev;     // previous block
	buffer_index next;     // next block
	buffer_index lines;     // first line of the block
	i32 n_lines;            // number of lines in that block
};
typedef struct block block;

inline i32 block_is_first(block *b)
{
	return b->prev.offset < 0;
}

inline i32 block_is_last(block *b)
{
	return b->next.offset < 0;
}

inline block* block_get_next(block *b, buffer *buf)
{
	return buffer_get_block(buf, b->next);
}

inline block* block_get_prev(block *b, buffer *buf)
{
	return buffer_get_block(buf, b->prev);
}

inline slice* block_get_line(block *b, i32 n, buffer *buf)
{
	assert(0 <= n);
	assert(n < b->n_lines);
	return buffer_get_line(buf, buffer_index_offset(b->lines, n));
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
	buffer_index new_blocks; // TODO: is this necessary ? In practice the new blocks will always be
	                         //       immediately assigned after that block_op.
	buffer_index old_block;
	buffer_index prev_op;
};

_def_buffer_alloc(line, struct slice);
_def_buffer_alloc(block, struct block);
_def_buffer_alloc(block_op, struct block_op);

/* A file mapped into memory for manipulation in the editor, + its metadata */
struct mapped_file {
	char name[256]; // '\0' terminated filepath
	struct stat file_stat;
	slice data;
	// TODO: mmap timestamp ?
};


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

	size_t len = f->file_stat.st_size;
	f->data.start = mmap(NULL, len, prot, flags, fd, offset);
	f->data.stop  = f->data.start + len;
	_fail_if(f->data.start == MAP_FAILED, "mapped_file_load(%s) failed: %s", f->name, strerror(errno));

	close(fd);
	return 0;
}

void mapped_file_print(int out_fd, struct mapped_file *f)
{
	slice s = f->data;
	while (slice_len(s)) {
		slice_write(out_fd, slice_split(&s, '\n'));
	}
}

// All data relative to a text file representation in memory and insert/delete ops for undo/redo.
struct filebuffer {
	// TODO: mapped_file.name should be stored somewhere else in some index of name -> filebuffer
	struct mapped_file         file;	        // Mapped file, read only
	struct buffer              insert_buffer;       // Insert buffer, append only.
	struct buffer              object_buffer;       // Object buffer for line slices, blocks, block_ops.
	buffer_index               last_op;             // Index into the buffer struct of the last ops.
	// TODO: add metadata for redo !

	// FIXME: convert to buffer_index
	buffer_index b_first;
	buffer_index b_last;
};
typedef struct filebuffer filebuffer;

// A cursor into a filebuffer that can be moved line by line.
// Careful: cursors get invalidated by inserts and deletes !
struct filebuffer_cursor {
	buffer *buf;
	block *blk;
	i32 cursor;
	i32 lineno;
};
typedef struct filebuffer_cursor filebuffer_cursor;

inline filebuffer_cursor filebuffer_cursor_init(filebuffer *fb)
{
	return (filebuffer_cursor) {
		.buf = &fb->object_buffer,
		.blk = buffer_get_block(&fb->object_buffer, fb->b_first),
		.cursor = -1,
		.lineno = -1,
	};
}

i32 filebuffer_cursor_next(filebuffer_cursor *it)
{
	block *b = it->blk;
	i32 c = ++it->cursor;
	it->lineno++;
	if (c == b->n_lines) {
		if (block_is_last(b)) {
			return 0;
		}
		it->blk = block_get_next(b, it->buf);
		it->cursor = 0;
	}
	return 1;
}

i32 filebuffer_cursor_prev(filebuffer_cursor *it)
{
	block *b = it->blk;
	i32 c = it->cursor--;
	it->lineno--;
	if (c < 0) {
		if (block_is_first(b)) {
			return 0;
		}
		it->blk = block_get_prev(b, it->buf);
		it->cursor = b->n_lines - 1;
	}
	return 1;
}

inline slice filebuffer_cursor_get(filebuffer_cursor *it)
{
	return *block_get_line(it->blk, it->cursor, it->buf);
}


// filebuffer TODOs:
//   - deleting a range of blocks
//   - inserting a block
//   - appending char to last insert
//   - locating blocks by line number
//   - undo/redo
//   - creating a cursor from given lineno

// Map a file in memory, scan content for finding all lines, prepare initial line block.
static u64 filebuffer_alloc_base = Giga(256);
static u64 filebuffer_alloc_span = Mega(64);
static i32 filebuffer_alloc_n = 0;

u64 filebuffer_init_next_addr() {
	u64 a = filebuffer_alloc_base + filebuffer_alloc_n * filebuffer_alloc_span;
	filebuffer_alloc_n ++;
	return a;
}

// TODO: do something better which allows to deallocate memory regions
//       and can grow dynamically, but no malloc !!
//       A few considerations
//       - in general we don't want to share the append buffers between filebuffer
//         because closing a file would mean GC'ing the append buffer
//       - line slices and blocks can be interleaved and share the same flat buffer
//       - if line slices and blocks are refered via relative indexes, the whole filebuffer data can be moved
//       - history buffer needs to be contiguous otherwise ops needs back and forth pointers ...
//       - if history is bounded, then the line and block buffer could be GCed ...
//
//       Taking into account all of that into considerations a filebuffer should boil down to
//         - the filename stored elsewhere in some kind of index
//         - the file original data, size is known
//         - the history buffer, can be bounded and fixed in size,
//         - the append buffer, grows at the end only
//         - the slice and block buffers, grows at the end only.
//
//       One possible solution
//         - reference all blocks and line via relative index
//         - link history ops together with relative index
//         - alloc a small per-file initial memory region
//         - bottom addresses are used in a forward fashion for the append buffer
//         - top addresses are used in a backward fashion for line slices, blocks, history ops
//         - when top and bottom are about to collide, move everything into a bigger block
//	     - actually only the top block can be moved up with a single memmove once the upper regions
//	     - is reallocated
void filebuffer_init(filebuffer *fb)
{
	u64 addr = filebuffer_init_next_addr();
	u64 base_append_buffer = addr;
	u64 base_object_buffer = addr + Mega(32);

	size_t len = Kilo(64);

	u8 *append_buffer_ptr = v_alloc(base_append_buffer, len);
	u8 *object_buffer_ptr = v_alloc(base_object_buffer, len);

	_fail_if(append_buffer_ptr == NULL, "filebuffer_alloc: v_alloc failed %lu\n", base_append_buffer);
	_fail_if(object_buffer_ptr == NULL, "filebuffer_alloc: v_alloc failed %lu\n", base_object_buffer);

	buffer object_buffer = buffer_mk(append_buffer_ptr , len);
	buffer insert_buffer = buffer_mk(object_buffer_ptr , len);
	fb->insert_buffer = object_buffer;
	fb->object_buffer = insert_buffer;

	int z;
	z = mapped_file_load(&fb->file);
	assert(z == 0);

	// Assign initial block of lines
	buffer_index first_block = buffer_alloc_block(&object_buffer);
	fb->b_first = first_block;
	fb->b_last = first_block;

	block *b = buffer_get_block(&object_buffer, first_block);
	b->prev = buffer_index_invalid;
	b->next = buffer_index_invalid;

	// Scan file for initial line slices and block setup.
	slice s = fb->file.data;
	while (slice_len(s)) {
		buffer_index l = buffer_alloc_line(&object_buffer);
		if (b->n_lines == 0) {
			b->lines = l;
		}
		b->n_lines++;

		*buffer_get_line(&object_buffer, l) = slice_split(&s, '\n');
	}
}

void filebuffer_save(filebuffer *fb, int fd)
{
	filebuffer_cursor c = filebuffer_cursor_init(fb);
	while (filebuffer_cursor_next(&c)) {
		slice_write(fd, filebuffer_cursor_get(&c));
	}
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






/* MAIN and TESTS */

static char buf[1024] = {};

void test_valloc()
{
	u64 addr_base = 0xffff000000;
	void* addr = v_alloc(addr_base, PageSize * 12*32);
	//void* addr = v_alloc(addr, 262144*32);
	_fail_if(addr == NULL, "v_alloc (mmap) failed: %s", strerror(errno));
}

void test_sizeof()
{
	print_sizeof(filebuffer);
	print_sizeof(buffer);
}

void test_mapped_file()
{
	struct mapped_file f = {
		.name =
			"/Users/hugobenichi/Desktop/editor/czl/Makefile",
			//__FILE__,
	};
	mapped_file_load(&f);
	mapped_file_print(STDOUT_FILENO, &f);
}

void test_vec_rec()
{
	rec r0 = r(v(1, 4), v(8, 9));
	vec v1 = v(5, 6);
	vec v2 = v(3, 2);

	rec r1 = add(r0, v1);
	rec r2 = add(v2, r1);
	vec v3 = add(v1, v2);

	vec v4 = sub(v3, v2);
	rec r3 = sub(r2, v3);

	*vec_print(buf, 1024, v1) = 0;     puts(buf);
	*vec_print(buf, 1024, v2) = 0;     puts(buf);
	*vec_print(buf, 1024, v3) = 0;     puts(buf);
	*vec_print(buf, 1024, v4) = 0;     puts(buf);

	*rec_print(buf, 1024, r0) = 0;     puts(buf);
	*rec_print(buf, 1024, r1) = 0;     puts(buf);
	*rec_print(buf, 1024, r2) = 0;     puts(buf);
	*rec_print(buf, 1024, r3) = 0;     puts(buf);

}

void test_filebuffer()
{
	struct filebuffer fb = {
		.file.name = "/Users/hugobenichi/Desktop/editor/czl/Makefile",
	};
	filebuffer_init(&fb);

	char file[256] = "/tmp/czl_Makefile.bkp";
	int fd = open(file, O_RDWR|O_CREAT, 0644);
	filebuffer_save(&fb, fd);
	close(fd);
}

void test_term()
{
	term_raw();
	struct key_input k = read_input();

	snprintf(buf, 1024, "read_input: %d\n", k.c);
	fputs(buf, logs);
	puts(buf);
}

void test_echo()
{
	for (;;) {
		char* end = key_print(buf, 1024, read_input());
		*end = 0;
		puts(buf);
	}
}

void test_inputcapture()
{
	slice s = {
		.start = (u8*) buf,
		.stop  = (u8*) buf + 1024,
	};
	slice input = input_capture(s);
	write(STDIN_FILENO, (char*) input.start, input.stop - input.start);
	puts("");
}

int main(int argc, char **args)
{
	logs = fopen(logs_path, "w");
	assert(logs > 0);
	fputs("hello chizel !!\n", logs);

	//test_valloc();
	//test_sizeof();
	//test_mapped_file();
	//test_vec_rec();
	test_filebuffer();
	//test_term();
	//test_echo();
	//test_inputcapture();

	fflush(logs);
	fclose(logs);
}

