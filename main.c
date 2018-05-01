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
		.n_lines = len / sizeof(slice), // remainder of ptr are wasted
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

inline i32 block_is_first(struct block *b)
{
	return !b->prev;
}

inline i32 block_is_last(struct block *b)
{
	return !b->next;
}

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
		.n_blocks  = len / sizeof(struct block_buffer), // remainder of ptr are wasted
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
		.n_ops_max = len / sizeof(struct block_op),
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
	struct mapped_file         f;    // Mapped file, read only
	struct buffer	           a;    // Buffer for insertions, append only.
	struct line_buffer         l;    // Buffer for line slices pointing into 'f' or 'a'
	struct block_buffer        b;    // Buffer for blocks of lines
	// TODO: consider making this a ring buffer with bounded history length
	struct block_op_history	   h;    // Buffer for history of block operations

	struct block *b_first;
	struct block *b_last;
};
typedef struct filebuffer filebuffer;

// A cursor into a filebuffer that can be moved line by line.
// Careful: cursors get invalidated by inserts and deletes !
struct filebuffer_cursor {
	struct block *b;
	i32 block_cursor;
	i32 lineno;
};

inline struct filebuffer_cursor filebuffer_cursor_init(struct filebuffer *fb)
{
	return (struct filebuffer_cursor) {
		.b = fb->b_first,
		.block_cursor = -1,
		.lineno = -1,
	};
}

i32 filebuffer_cursor_next(struct filebuffer_cursor *c)
{
	c->lineno++;
	c->block_cursor++;
	if (c->block_cursor == c->b->n_lines) {
		if (block_is_last(c->b)) {
			return 0;
		}
		c->b = c->b->next;
		c->block_cursor = 0;
	}
	return 1;
}

i32 filebuffer_cursor_prev(struct filebuffer_cursor *c)
{
	c->lineno--;
	c->block_cursor--;
	if (c->block_cursor == -1) {
		if (block_is_first(c->b)) {
			return 0;
		}
		c->b = c->b->prev;
		c->block_cursor = c->b->n_lines - 1;
	}
	return 1;
}

inline slice filebuffer_cursor_get(struct filebuffer_cursor *c)
{
	return *(c->b->lines + c->block_cursor);
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
static u64 filebuffer_alloc_span = Mega(8);
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
void filebuffer_init(struct filebuffer *fb)
{
	u64 addr = filebuffer_init_next_addr();
	u8 *ptr = v_alloc(addr, filebuffer_alloc_span);
	_fail_if(ptr == NULL, "filebuffer_alloc: v_alloc failed %lu\n", addr);

	u8 *base_history_buffer       = ptr;
	u8 *base_line_buffer          = ptr + Mega(2);
	u8 *base_block_buffer         = ptr + Mega(4);
	u8 *base_append_buffer        = ptr + Mega(6);

	fb->a = buffer_mk(base_append_buffer, Mega(2));
	fb->l = line_buffer_mk(base_line_buffer, Mega(2));
	fb->b = block_buffer_mk(base_block_buffer, Mega(2));
	fb->h = block_op_history_mk(base_history_buffer, Mega(2));

	int z;
	z = mapped_file_load(&fb->f);
	assert(z == 0);

	slice s = fb->f.data;
	while (slice_len(s)) {
		*line_buffer_assign(&fb->l) = slice_split(&s, '\n');
	}

	// Link initial blocks together
	struct block *b = block_buffer_assign(&fb->b);
	fb->b_first = b;
	fb->b_last = b;

	// Assign initial block of lines
	b->lines = fb->l.lines;
	b->n_lines = line_buffer_used(&fb->l);
	b->prev = NULL;
	b->next = NULL;
}

void filebuffer_save(struct filebuffer *fb, int fd)
{
	struct filebuffer_cursor c = filebuffer_cursor_init(fb);
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







#define print_sizeof(str_name) 	printf("sizeof(%s)= %luB\n", #str_name, sizeof(str_name))

int main(int argc, char **args)
{
	logs = fopen(logs_path, "w");
	assert(logs > 0);

	printf("logs address: %p\n", &logs);

	print_sizeof(struct filebuffer);
	print_sizeof(struct buffer);
	print_sizeof(struct block_op);

	puts("hello chizel !!");
	fputs("hello chizel !!\n", logs);

	u64 addr_base = 0xffff000000;
	void* addr = v_alloc(addr_base, PageSize * 12*32);
	//void* addr = v_alloc(addr, 262144*32);
	_fail_if(addr == NULL, "v_alloc (mmap) failed: %s", strerror(errno));

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

	print_sizeof(struct block);
	print_sizeof(struct slice);

//	for (;;) {
//		char* end = key_print(buf, 1024, read_input());
//		*end = 0;
//		puts(buf);
//	}

	struct filebuffer fb = {
		.f.name = "/Users/hugobenichi/Desktop/editor/czl/Makefile",
	};
	filebuffer_init(&fb);

	char file[256] = "/tmp/czl_Makefile.bkp";
	int fd = open(file, O_RDWR|O_CREAT, 0644);
	filebuffer_save(&fb, fd);
	close(fd);

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
