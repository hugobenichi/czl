typedef unsigned char u8;

int min(int a, int b) {
  return a < b ? a : b;
};

#include <limits.h>
static const int no_lineno = INT_MIN;

struct vec {
  int x;
  int y;
};

struct slice {
  u8 *start;
  u8 *stop;
};

struct text_view {
  int           n;
  int*          linenos;
  struct slice* lines;
  int*          lines_len;
  struct vec    cursor;
  struct vec    offset;
};

struct vec cursor_pos(struct cursor*);
struct slice cursor_line_get(struct cursor*);
int cursor_line_next(struct cursor*);

int line_len(u8* line) {
  u8* end = rawmemchr(line, '\n');
  if (end == line) {
    return 0;
  }
  if (*(end - 1) == '\r') {
    end--;
  }
  return end - line;
}

int slice_len(struct slice s) {
  return s.stop - s.start;
}

struct slice slice_drop(struct slice s, int n) {
  n = min(n, slice_len(s));
  s.start = s.start - n;
  return s;
}

void textview_fill(struct text_view *v, struct cursor *c) {
  struct vec pos = cursor_pos(c);
  v->cursor = pos;
  int lineno_base = 1 + pos.y;

  for (int i = 0; i < v->n; i++) {
    struct slice line = cursor_line_get(c);

    v->lines[i]     = line;
    v->linenos[i]   = lineno_base + i;
    v->lines_len[i] = slice_len(line);

    if (!cursor_line_next(c)) {
      return;
    }
  }
}

void textview_set_relative_lineno(struct text_view *v) {
  int v_offset = 1 + v->cursor.y;

  for (int i = 0; i < v->n; i++)
    if (v->linenos[i]!= no_lineno)
      v->linenos[i] -= v_offset;
}

void textview_set_hoffset(struct text_view *v, int h_offset) {
  for (int i = 0; i < v->n; i++)
    v->lines[i] = slice_drop(v->lines[i], h_offset);
}
