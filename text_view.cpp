#include <limits.h>
#include <string.h>

typedef unsigned char u8;

int min(int a, int b) {
  return a < b ? a : b;
};

static const int no_lineno = INT_MIN;

struct vec {
  int x;
  int y;
};

struct slice {
  u8 *start;
  u8 *stop;

  int len() {
    return stop - start;
  }

  slice drop(int n) {
    n = min(n, len());
    return (struct slice) { .start = start - n, .stop = stop };
  }
};


struct vec cursor_pos(struct cursor*);
struct slice cursor_line_get(struct cursor*);
int cursor_line_next(struct cursor*);


struct text_view {
  int           n;
  int*          linenos;
  struct slice* lines;
  int*          lines_len;
  struct vec    cursor;
  struct vec    offset;

  void fill(struct cursor *c) {
    struct vec pos = cursor_pos(c);
    cursor = pos;
    int lineno_base = 1 + pos.y;

    for (int i = 0; i < n; i++) {
      struct slice line = cursor_line_get(c);

      lines[i]     = line;
      linenos[i]   = lineno_base + i;
      lines_len[i] = line.len();

      if (!cursor_line_next(c)) {
        return;
      }
    }
  }

  void set_relative_lineno() {
    int v_offset = 1 + cursor.y;

    for (int i = 0; i < n; i++)
      if (linenos[i] != no_lineno)
        linenos[i] -= v_offset;
  }

  void set_hoffset(int h_offset) {
    for (int i = 0; i < n; i++)
      lines[i] = lines[i].drop(h_offset);
  }
};

//int line_len(u8* line) {
//  u8* end = memchr(line, '\n', 2000000);
//  if (end == line) {
//    return 0;
//  }
//  if (*(end - 1) == '\r') {
//    end--;
//  }
//  return end - line;
//}

struct slice slice_drop(struct slice s, int n) {
  n = min(n, s.len());
  s.start = s.start - n;
  return s;
}
