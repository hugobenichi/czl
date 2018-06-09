#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]



use std::fs;
use std::io;
use std::io::prelude::*;
use std::str;
use std::result;
use std::cmp::min;
use std::cmp::max;
use std::mem::replace;

// FIXME bug with fileno offset when scrolling up ??
// FIXME bug with cursor and delete position not in sync !!

/*
 * Next Steps:
 *  - add text insert
 *      commands: new line, line copy, insert mode, append char
 *
 * General TODOs:
 *  - handle resize
 *  - dir explorer
 *  - frame latency stats
 *  - command mode pending input
 *  - think more about where to track the screen area:
 *      right now it is repeated both in Screen and in Fileview
 *      ideally Screen would not be tracking it
 *  - systematically propagate errors everywhere
 */


// Global constant that controls a bunch of options.
const CONF : Config = Config {
    draw_screen:            true,
    draw_colors:            true,
    retain_frame:           false,
    no_raw_mode:            false, //true,

    debug_console:          true,
    debug_bounds:           true,

    relative_lineno:        true,
    cursor_show_line:       true,
    cursor_show_column:     true,

    color_default:          Colorcell { fg: Color::Black,   bg: Color::White },
    color_header_active:    Colorcell { fg: Color::Gray(2), bg: Color::Yellow },
    color_header_inactive:  Colorcell { fg: Color::Gray(2), bg: Color::Cyan },
    color_footer:           Colorcell { fg: Color::White,   bg: Color::Gray(2) },
    color_lineno:           Colorcell { fg: Color::Green,   bg: Color::Gray(2) },
    color_console:          Colorcell { fg: Color::White,   bg: Color::Gray(16) },
    color_mode_command:     Colorcell { fg: Color::Gray(1), bg: Color::Black },
    color_mode_insert:      Colorcell { fg: Color::Gray(1), bg: Color::Red },
    color_cursor_lines:     Colorcell { fg: Color::Black,   bg: Color::Gray(6) },
};


// TODO: experiment with a static framebuffer that has a &mut[u8] instead of a vec.


/* CORE TYPE DEFINITION */

// The core editor structure
struct Editor {
    window:         Vek,              // The dimensions of the editor and backend terminal window
    mainscreen:     Rec,              // The screen area for displaying file content and menus.
    footer:         Rec,
    framebuffer:    Framebuffer,

    mode:           EditorMode,

    running: bool,
    // TODO:
    //  list of open files and their filebuffers
    //  list of screens
    //  current screen layout
    //  Mode state machine

    // For the time being, only one file can be loaded and edited per program.
    filebuffer: Filebuffer,
    fileview:   Fileview,
}


struct Config {
    draw_screen:            bool,
    draw_colors:            bool,
    retain_frame:           bool,
    no_raw_mode:            bool,

    debug_console:          bool,
    debug_bounds:           bool,

    relative_lineno:        bool,
    cursor_show_line:       bool,
    cursor_show_column:     bool,

    color_default:          Colorcell,
    color_header_active:    Colorcell,
    color_header_inactive:  Colorcell,
    color_footer:           Colorcell,
    color_lineno:           Colorcell,
    color_console:          Colorcell,
    color_mode_command:     Colorcell,
    color_mode_insert:      Colorcell,
    color_cursor_lines:     Colorcell,
}

// Either a position in 2d space w.r.t to (0,0), or a movement quantity
#[derive(Debug, Clone, Copy)]
struct Vek { // Vec was already taken ...
    x: i32,
    y: i32,
}

// A simple rectangle
// In general, the top-most raw and left-most column should be inclusive (min),
// and the bottom-most raw and right-most column should be exclusive (max).
#[derive(Debug, Clone, Copy)]
struct Rec {
    min: Vek,   // point the closest to (0,0)
    max: Vek,   // point the farthest to (0,0)
}

type Colorcode = i32;

#[derive(PartialEq, Debug, Clone, Copy)]
enum Color {
    /* First 8 ansi colors */
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    /* High contrast 8 ansi colors */
    BoldBlack,
    BoldRed,
    BoldGreen,
    BoldYellow,
    BoldBlue,
    BoldMagenta,
    BoldCyan,
    BoldWhite,
    /* 6 x 6 x 6 RGB colors = 216 colors */
    RGB216 { r: i32, g: i32, b: i32 },
    /* 24 level of Grays */
    Gray(i32),
}

#[derive(Debug, Clone, Copy)]
struct Colorcell {
    fg: Color,
    bg: Color,
}

fn colorcell(fg: Color, bg: Color) -> Colorcell {
    Colorcell { fg, bg }
}

#[derive(Debug)]
enum Move {
    Left,
    Right,
    Up,
    Down,
    Start,
    End,
}

#[derive(Debug)]
enum MovementMode {
    Chars,
    Lines,
    Blocks,
    Words,
    Digits,
    Numbers,
    Paragraphs,
    Parens,
    Brackets,
    Braces,
    Selection,
    Pages,
}

#[derive(Debug, Clone, Copy)]
enum EditorMode {
    Command,
    Insert,
}

const MODE_COMMAND : &'static str = "Command";
const MODE_INSERT  : &'static str = "Insert ";

impl EditorMode {
    fn footer_color(self) -> Colorcell {
        match self {
            EditorMode::Command =>  CONF.color_mode_command,
            EditorMode::Insert  =>  CONF.color_mode_insert,
        }
    }

    fn name(self) -> &'static str {
        match self {
            EditorMode::Command =>  MODE_COMMAND,
            EditorMode::Insert  =>  MODE_INSERT,
        }
    }
}

// The struct that manages compositing.
struct Framebuffer {
    window:     Vek,
    len:        i32,

    text:       Vec<u8>,
                // TODO: store u8 instead and use two tables
                // for color -> u8 -> control string conversions
    fg:         Vec<Color>,
    bg:         Vec<Color>,
    cursor:     Vek,

    buffer:     Bytebuffer,
}

// Append only buffer with a cursor
struct Bytebuffer {
    bytes:  Vec<u8>,
    cursor: usize,
}

// Transient object for putting text into a subrectangle of a framebuffer.
// Since it needs a mut ref to the framebuffer, Screen objs cannot be stored.
struct Screen<'a> {
    framebuffer:    &'a mut Framebuffer,
    window:         Rec,
    linenoarea:     Rec,
    textarea:       Rec,
    header:         Rec,
    fileview:       &'a Fileview,
    // TODO: consider adding Filebuffer directly here too
}

enum Draw {
    Nothing,
    All,
    Header,
    Text,
}

struct Textbuffer {
    text:   Vec<u8>,    // original content of the file when loaded
    lines:  Vec<Line>,  // subslices into the filebuffer or appendbuffer
}

#[derive(Clone)]
struct Textsnapshot {
    line_indexes: Vec<usize> // the actual lines in the current files, as indexes into 'lines'
}

// Manage content of a file
// Q: can I have a vec in a struct and another subslice pointing into that vec ?
//    I would need to say that they both have the same lifetime as the struct.
struct Filebuffer {
    textbuffer: Textbuffer,
    previous_snapshots: Vec<Textsnapshot>,
    current_snapshot: Textsnapshot,
}

// A pair of offsets into a filebuffer for delimiting lines.
#[derive(Debug, Clone, Copy)]
struct Line {
    start:  usize,      // inclusive
    stop:   usize,      // exclusive
}

#[derive(Debug, Clone, Copy)]
struct Linerange {
    // TODO: would that be useful to represent a file as a list of line range ?
    //       what about fragmentation after a while ? Aren't Lineranges going to collapse to single
    //       line ranges ?
    start:  usize,
    stop:   usize,
}

// Point to a place inside a Filebuffer
struct Cursor<'a> {
    filebuffer: &'a Filebuffer,
}

// Store states related to navigation in a given file.
struct Fileview {
    filepath:           String,
    relative_lineno:    bool,
    movement_mode:      MovementMode,
    show_token:         bool,
    show_neighbor:      bool,
    show_selection:     bool,
    is_active:          bool,
    cursor:             Vek,
    filearea:           Rec,
    //selection:  Option<&[Selection]>
}

impl Fileview {
    fn mk_fileview(filepath: String, screensize: Vek) -> Fileview {
        Fileview {
            filepath,
            relative_lineno:    CONF.relative_lineno,
            movement_mode:      MovementMode::Chars,
            show_token:         false,
            show_neighbor:      false,
            show_selection:     false,
            is_active:          true,
            cursor:             vek(0,0),
            filearea:           screensize.rec(),
        }
    }

    fn update(&mut self, filebuffer: &Filebuffer) {
        // TODO: this should track a 'desired cursor position' and take this into
        // account when adjusting the real cursor position.
        let mut p = self.cursor;

        p.y = min(p.y, filebuffer.nlines() - 1);
        p.y = max(0, p.y);

        // Right bound clamp pushes x to -1 for empty lines.
        p.x = min(p.x, filebuffer.line_len(p.y as usize) as i32 - 1);
        p.x = max(0, p.x);

        self.cursor = p;

        let mut dx = 0;
        let mut dy = 0;

        if p.y < self.filearea.min.y {
            dy = p.y - self.filearea.min.y;
        }
        if self.filearea.max.y <= p.y {
            dy = p.y + 1 - self.filearea.max.y;
        }
        if p.x < self.filearea.min.x {
            dx = p.x - self.filearea.min.x;
        }
        if self.filearea.max.x <= p.x {
            dx = p.x + 1 - self.filearea.max.x;
        }

        self.filearea = self.filearea + vek(dx, dy);
        // TODO: add horizontal scrolling !
    }
}


// + everything needed for input processing ...




/* CORE TYPES IMPLS */

fn vek(x: i32, y: i32) -> Vek {
    Vek { x, y }
}

// TODO: rec ctor with width and height ??
fn rec(x0: i32, y0: i32, x1: i32, y1: i32) -> Rec {
    let (a0, a1) = ordered(x0, x1);
    let (b0, b1) = ordered(y0, y1);
    Rec {
        min: vek(a0, b0),
        max: vek(a1, b1),
    }
}


impl Rec {
    fn x0(self) -> i32 { self.min.x }
    fn y0(self) -> i32 { self.min.y }
    fn x1(self) -> i32 { self.max.x }
    fn y1(self) -> i32 { self.max.y }
    fn w(self) -> i32 { self.max.x - self.min.x }
    fn h(self) -> i32 { self.max.y - self.min.y }

    fn area(self) -> i32 { self.w() * self.h() }
    fn size(self) -> Vek { vek(self.w(), self.h()) }

    fn row(self, y: i32) -> Rec {
        assert!(self.min.y <= y);
        assert!(y <= self.max.y);
        rec(self.min.x, y, self.max.x, y + 1)
    }

    fn column(self, x: i32) -> Rec {
        assert!(self.min.x <= x);
        assert!(x <= self.max.x);
        rec(x, self.min.y, x + 1, self.max.y)
    }

    // TODO: should x be forbidden from matching the bounds (i.e no empty output)
    fn hsplit(self, x: i32) -> (Rec, Rec) {
        assert!(self.min.x <= x);
        assert!(x < self.max.x);

        let left = rec(self.min.x, self.min.y, x, self.max.y);
        let right = rec(x, self.min.y, self.max.x, self.max.y);

        (left, right)
    }

    fn vsplit(self, y: i32) -> (Rec, Rec) {
        assert!(self.min.y <= y);
        assert!(y < self.max.y);

        let up = rec(self.min.x, self.min.y, self.max.x, y);
        let down = rec(self.min.x, y, self.max.x, self.max.y);

        (up, down)
    }

    // TODO: add a hsplit function
}


/* Vek/Vek ops */

impl Vek {
    fn rec(self) -> Rec {
        Rec {
            min: vek(0,0),
            max: self,
        }
    }

    fn extrude(self, diag: Vek) -> Rec {
        Rec {
            min: self,
            max: self + diag,
        }
    }
}

impl std::ops::Add<Vek> for Vek {
    type Output = Vek;

    fn add(self, v: Vek) -> Vek {
        vek(self.x + v.x, self.y + v.y)
    }
}

impl std::ops::Sub<Vek> for Vek {
    type Output = Vek;

    fn sub(self, v: Vek) -> Vek {
        vek(self.x - v.x, self.y - v.y)
    }
}

impl std::ops::Neg for Vek {
    type Output = Vek;

    fn neg(self) -> Vek {
        vek(-self.x, -self.y)
    }
}

/* Vek/Rec ops */

impl Rec {
    // TODO: consider excluding max
    fn contains(self, v : Vek) -> bool {
        self.min.x <= v.x &&
        self.min.y <= v.y &&
                      v.x <= self.max.x &&
                      v.y <= self.max.y
    }
}

impl std::ops::Add<Vek> for Rec {
    type Output = Rec;

    fn add(self, v: Vek) -> Rec {
        Rec {
            min: self.min + v,
            max: self.max + v,
        }
    }
}

impl std::ops::Add<Rec> for Vek {
    type Output = Rec;

    fn add(self, r: Rec) -> Rec {
        r + self
    }
}

impl std::ops::Sub<Vek> for Rec {
    type Output = Rec;

    fn sub(self, v: Vek) -> Rec {
        Rec {
            min: self.min - v,
            max: self.max - v,
        }
    }
}


/* Colors */

fn colorcode(c : Color) -> Colorcode {
    match c {
        // TODO !
        Color::Black                    => 0,
        Color::Red                      => 1,
        Color::Green                    => 2,
        Color::Yellow                   => 3,
        Color::Blue                     => 4,
        Color::Magenta                  => 5,
        Color::Cyan                     => 6,
        Color::White                    => 7,
        Color::BoldBlack                => 8,
        Color::BoldRed                  => 9,
        Color::BoldGreen                => 10,
        Color::BoldYellow               => 11,
        Color::BoldBlue                 => 12,
        Color::BoldMagenta              => 13,
        Color::BoldCyan                 => 14,
        Color::BoldWhite                => 15,
        Color::RGB216 { r, g, b }       => 15 + (r + 6 * (g + 6 * b)),
        Color::Gray(g)                  => 255 - g,
    }
}


/* UTILITIES */

fn itoa10(dst: &mut [u8], x: i32, padding: u8) {
    fill(dst, padding);
    let mut y = x.abs();
    let mut idx = dst.len() - 1;
    loop {
        let b = (y % 10) as u8 + '0' as u8;
        dst[idx] = b;

        y /= 10;
        if y == 0 {
            break;
        }

        if idx == 0 {
            // overflows trap by default !
            return;
        }
        idx -= 1;
    }
    if x < 0 {
        dst[idx - 1] = '-' as u8;
    }
}

// Because lame casting syntax
fn usize(x: i32) -> usize {
    x as usize
}

fn ordered<T>(v1: T, v2: T) -> (T, T) where T : Ord {
    if v1 < v2 {
        return (v1, v2)
    }
    (v2, v1)
}

fn reorder<T>(v1: &mut T, v2: &mut T) where T : Ord {
    if v1 > v2 {
        std::mem::swap(v1, v2)
    }
}

// CLEANUP: replace with memset if this is ever a thing in Rust
fn fill<T>(s: &mut [T], t: T) where T : Copy {
    for i in s.iter_mut() {
        *i = t
    }
}

fn copy_exact<T>(dst: &mut [T], src: &[T]) where T : Copy {
    dst.clone_from_slice(src)
}

fn copy<T>(dst: &mut [T], src: &[T]) where T : Copy {
    let n = min(dst.len(), src.len());
    copyn(dst, src, n)
}

fn copyn<T>(dst: &mut [T], src: &[T], n: usize) where T : Copy {
    dst[..n].clone_from_slice(&src[..n])
}

// CLEANUP: replace with std memchr when this make it into stable
fn memchr(c: u8, s: &[u8]) -> Option<usize> {
    for (i, &x) in s.iter().enumerate() {
        if x == c {
            return Some(i)
        }
    }
    None
}

fn clamp<'a, T>(s: &'a[T], l: usize) -> &'a[T] {
    &s[..min(l, s.len())]
}

fn shift<'a, T>(s: &'a[T], o: usize) -> &'a[T] {
    &s[min(o, s.len())..]
}

fn subslice<'a, T>(s: &'a[T], offset: usize, len: usize) -> &'a[T] {
    clamp(shift(s, offset), len)
}

/* CORE TYPE IMPLEMENTATION */


impl Bytebuffer {
    fn mk_bytebuffer() -> Bytebuffer {
        Bytebuffer {
            bytes:  vec![0; 64 * 1024],
            cursor: 0,
        }
    }

    fn rewind(&mut self) {
        self.cursor = 0
    }

    fn append(&mut self, src: &[u8]) {
        let dst = &mut self.bytes;
        let l = src.len();
        let c1 = self.cursor;
        let c2 = c1 + l;
        if c2 > dst.capacity() {
            dst.reserve(l);
        }
        copy_exact( &mut dst[c1..c2], src);
        self.cursor = c2;
    }

    // TODO: propagate error
    fn write_into<T>(&self, t: &mut T) where T : io::Write {
        let c = self.cursor;
        let d = &self.bytes[0..c];
        let n = t.write(d).unwrap();
        t.flush().unwrap();
        assert_eq!(n, c);
    }
}

const frame_default_text : u8 = ' ' as u8;
const frame_default_fg : Color = Color::Black;
const frame_default_bg : Color = Color::White;

impl Framebuffer {
    fn mk_framebuffer(window: Vek) -> Framebuffer {
        let len = window.x * window.y;
        let vlen = len as usize;

        Framebuffer {
            window,
            len,
            text:       vec![frame_default_text; vlen],
            fg:         vec![frame_default_fg; vlen],
            bg:         vec![frame_default_bg; vlen],
            cursor:     vek(0,0),
            buffer:     Bytebuffer::mk_bytebuffer(),
        }
    }

    // TODO: add clear in sub rec
    fn clear(&mut self) {
        fill(&mut self.text, frame_default_text);
        fill(&mut self.fg,   frame_default_fg);
        fill(&mut self.bg,   frame_default_bg);
    }

    fn put_line(&mut self, pos: Vek, src: &[u8]) {
        assert!(self.window.rec().contains(pos));

        let maxlen = (self.window.x - pos.x) as usize;
        let len = min(src.len(), maxlen);

        let start = (pos.y * self.window.x + pos.x) as usize;
        let stop = start + len;

        copy_exact(&mut self.text[start..stop], &src[..len]);
    }

    // area.min is inclusive, area.max is exclusive
    fn put_color(&mut self, area: Rec, colors: Colorcell) {
        if CONF.debug_bounds {
            assert!(0 <= area.x0());
            assert!(0 <= area.y0());
            assert!(area.x1() <= self.window.x);
            assert!(area.y1() <= self.window.y);
        }

        let y0 = max(0, area.y0());
        let y1 = min(area.y1(), self.window.y);

        let dx = self.window.x as usize;
        let xbase = dx * usize(y0);
        let mut x0 = xbase + max(0, area.x0()) as usize;
        let mut x1 = xbase + min(area.x1(), self.window.x) as usize;

        for i in y0..y1 {
            fill(&mut self.fg[x0..x1], colors.fg);
            fill(&mut self.bg[x0..x1], colors.bg);
            x0 += dx;
            x1 += dx;
        }
    }

    fn set_cursor(&mut self, new_cursor: Vek) {
        let mut x = new_cursor.x;
        let mut y = new_cursor.y;
        x = max(x, 0);
        x = min(x, self.window.x - 1);
        y = max(y, 0);
        y = min(y, self.window.y - 1);
        self.cursor = vek(x,y);
    }

    // TODO: propagate error
    // TODO: add color
    // PERF: skip unchanged sections
    fn push_frame(&mut self) {
        if !CONF.draw_screen {
            return
        }

        unsafe {
            CONSOLE.write_into(self);
        }

        self.buffer.rewind();
        self.buffer.append(term_cursor_hide);
        self.buffer.append(term_gohome);

        let w = self.window.x as usize;
        let mut l = 0;
        let mut r = w;
        for i in 0..self.window.y {
            if i > 0 {
                // Do not put "\r\n" on the last line
                self.buffer.append(term_newline);
            }

            if CONF.draw_colors {
                let mut j = l;
                loop {
                    let k = self.find_color_end(j, r);

                    // PERF: better color command creation without multiple string allocs.
                    let fg_code = colorcode(self.fg[j]);
                    let bg_code = colorcode(self.bg[j]);
                    self.buffer.append(format!("\x1b[38;5;{};48;5;{}m", fg_code, bg_code).as_bytes());

                    self.buffer.append(&self.text[j..k]);
                    if k == r {
                        break;
                    }
                    j = k;
                }
            } else {
                self.buffer.append(&self.text[l..r]);
            }

            l += w;
            r += w;
        }

        // Terminal cursor coodinates start at (1,1)
        let cursor_command = format!("\x1b[{};{}H", self.cursor.y + 1, self.cursor.x + 1);
        self.buffer.append(cursor_command.as_bytes());
        self.buffer.append(term_cursor_show);

        let stdout = io::stdout();
        self.buffer.write_into(&mut stdout.lock());
    }

    fn find_color_end(&self, a: usize, stop: usize) -> usize {
        let mut b = a;
        while b < stop && self.fg[a] == self.fg[b] && self.bg[a] == self.bg[b] {
            b += 1;
        }
        b
    }
}


fn log(msg: &str) {
    if !CONF.debug_console {
        return
    }
    unsafe {
        CONSOLE.log(msg);
    }
}

// For the sake of simplicity, this is not wrapped into a thread_local!(RefCell::new(...)).
static mut CONSOLE : Debugconsole = Debugconsole {
    width:      48,
    height:     16,
    next_entry: 0,
    text:       [0; 48 * 16],
};

struct Debugconsole {
    width:      i32,
    height:     i32,
    next_entry: i32,
    text:       [u8; 16 * 48],
}

impl Debugconsole {
    fn clear() {
        unsafe {
            CONSOLE.next_entry = 0;
        }
    }

    fn get_line<'a>(&'a self, i: i32) -> &'a [u8] {
        let src_start = usize(self.width * (i % self.height));
        let src_stop = src_start + usize(self.width);
        &self.text[src_start..src_stop]
    }

    fn get_line_mut<'a>(&'a mut self, i: i32) -> &'a mut [u8] {
        let src_start = usize(self.width * (i % self.height));
        let src_stop = src_start + usize(self.width);
        &mut self.text[src_start..src_stop]
    }

    fn log(&mut self, msg: &str) {
        let i = self.next_entry;
        self.next_entry += 1;
        let line = self.get_line_mut(i);
        fill(line, ' ' as u8);
        copy(line, msg.as_bytes());
    }

    fn write_into(&self, framebuffer: &mut Framebuffer) {
        if !CONF.debug_console {
            return
        }

        let size = vek(self.width, min(self.next_entry, self.height));
        let consoleoffset = - vek(0,1); // don't overwrite the footer.
        let consolearea = Rec { min: framebuffer.window - size, max: framebuffer.window } + consoleoffset;

        let start = max(0, self.next_entry - self.height);
        for i in start..self.next_entry {
            let dst_offset = consolearea.max - vek(self.width, self.next_entry - i);
            framebuffer.put_line(dst_offset, self.get_line(i));
        }
        framebuffer.put_color(consolearea, CONF.color_console);
    }
}


impl<'a> Screen<'a> {
    fn mk_screen<'b>(window: Rec, framebuffer: &'b mut Framebuffer, fileview: &'b Fileview) -> Screen<'b> {
        let lineno_len = 5;
        let (header, filearea) = window.vsplit(1);
        let (linenoarea, textarea) = filearea.hsplit(lineno_len);

        Screen {
            framebuffer,
            window,
            linenoarea,
            textarea,
            header,
            fileview,
        }
    }

    fn draw(&mut self, draw: Draw, filebuffer: &Filebuffer) {
        // TODO: use draw and only redraw what's needed
        // TODO: automatize the screen coordinate offsetting for framebuffer commands
        let fileoffset = self.fileview.filearea.min;

        // header
        {
                let header_string = format!("{}  {:?}", self.fileview.filepath, self.fileview.movement_mode);
                self.framebuffer.put_line(self.header.min, header_string.as_bytes());
                self.framebuffer.put_color(self.header, CONF.color_header_active);
        }

        // filebuffer content
        {
            let y_stop = min(self.textarea.h(), filebuffer.nlines() - fileoffset.y);
            for i in 0..y_stop {
                let lineoffset = vek(0, i);
                let text_offset = fileoffset + lineoffset;
                let frame_offset = self.textarea.min + lineoffset;

                let mut line = filebuffer.get_line(text_offset);
                line = clamp(line, self.textarea.w() as usize);
                self.framebuffer.put_line(frame_offset, line);
            }
        }

        // lineno
        {
            // TODO: add fileoffset !
            let mut buf = [0 as u8; 4];
            let lineno_base = if self.fileview.relative_lineno {
                -self.framebuffer.cursor.y
            } else {
                fileoffset.y + 1
            };
            for i in 0..self.textarea.h() {
                itoa10(&mut buf, lineno_base + i, ' ' as u8);
                self.framebuffer.put_line(self.linenoarea.min + vek(0,i), &buf);
            }
            self.framebuffer.put_color(self.linenoarea, CONF.color_lineno);
        }

        {
            let cursor_screen_position = self.fileview.cursor + self.textarea.min - fileoffset;
            if self.fileview.is_active {
                self.framebuffer.set_cursor(cursor_screen_position);
            }

            self.framebuffer.put_color(self.textarea.row(cursor_screen_position.y), CONF.color_cursor_lines);
            self.framebuffer.put_color(self.textarea.column(cursor_screen_position.x), CONF.color_cursor_lines);
        }
    }
}


impl Line {
    fn to_slice<'a>(self, text: &'a[u8]) -> &'a[u8] {
        &text[self.start..self.stop]
    }

    fn len(self) -> usize {
        self.stop - self.start // BUG: not utf8 aware ! no tab aware !
    }
}

impl Filebuffer {
    fn from_file(text: Vec<u8>) -> Filebuffer {

        let mut lines = Vec::new();
        let mut line_indexes = Vec::new();

        {
            // TODO: try using split iterator
            //for (i, line) in buf.split(|c| *c == newline).enumerate() {
            //    println!("{}: {}", i, str::from_utf8(line).unwrap())
            //}

            let l = text.len();
            let mut a = 0;
            while a < l {
                let b = match memchr('\n' as u8, &text[a..]) {
                    Some(o) => a + o,
                    None    => l,
                };
                lines.push(Line { start: a, stop: b });
                a = b + 1; // skip the '\n'
            }
        }

        for i in 0..lines.len() {
            line_indexes.push(i);
        }

        Filebuffer {
            textbuffer:         Textbuffer { text, lines },
            previous_snapshots: Vec::new(),
            current_snapshot:   Textsnapshot { line_indexes },
        }
    }

    // TODO: propagate errors
    fn to_file(&self, path: &str) {
        let mut f = fs::File::create(path).unwrap();

        for i in 0..self.nlines() {
            f.write_all(self.get_line(vek(0,i))).unwrap();
            f.write_all(b"\n").unwrap(); // TODO: use platform's newline
        }
    }

    fn nlines(&self) -> i32 {
        self.current_snapshot.line_indexes.len() as i32
    }

    fn line_len(&self, y: usize) -> usize {
        let idx = self.current_snapshot.line_indexes[y as usize];
        self.textbuffer.lines[idx].len()
    }

    fn get_line<'a>(&'a self, offset: Vek) -> &'a[u8] {
        let x = offset.x as usize;
        let y = offset.y as usize;
        let line_idx = self.current_snapshot.line_indexes[y];
        let line = self.textbuffer.lines[line_idx].to_slice(&self.textbuffer.text);
        shift(line, x)
    }
}


// Filebuffer ops
impl Filebuffer {

    fn line_del(&mut self, y: usize) {
        assert!(y < self.current_snapshot.line_indexes.len());

        let next_snapshot = self.current_snapshot.clone();
        let prev_snapshot = replace(&mut self.current_snapshot, next_snapshot);
        self.previous_snapshots.push(prev_snapshot);
        self.current_snapshot.line_indexes.remove(y);
    }

    fn undo(&mut self) {
        match self.previous_snapshots.pop() {
            Some(prev_snapshot) => self.current_snapshot = prev_snapshot,
            None => (),
        }
    }

    fn append(&mut self, c: u8) {
        // TODO
    }
}


impl Editor {

    fn mk_editor(filename: String, filebuffer: Filebuffer) -> Editor {
        let window = Term::size();
        let framebuffer = Framebuffer::mk_framebuffer(window);
        let running = true;
        let (mainscreen, footer) = window.rec().vsplit(window.y - 1);
        let fileview;
        {
            // reuse code in mk_Screen !!!
            let (_, filearea) = mainscreen.vsplit(1);
            let (_, textarea) = filearea.hsplit(5);
            fileview = Fileview::mk_fileview(filename, textarea.size());
        }
        let mode = EditorMode::Command;

        Editor {
            window,
            mainscreen,
            footer,
            framebuffer,
            mode,
            running,
            filebuffer,
            fileview,
        }
    }

    fn run(&mut self) {
        while self.running {
            self.refresh_screen();
            self.process_input();
        }
    }

    fn refresh_screen(&mut self) {
        {
            let mut screen = Screen::mk_screen(self.mainscreen, &mut self.framebuffer, &self.fileview);

            screen.draw(Draw::All, &self.filebuffer);
        }

        {
            self.framebuffer.put_line(self.footer.min + vek(1,0), self.mode.name().as_bytes());
            self.framebuffer.put_line(self.footer.min + vek(10, 0), b"FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER");
            self.framebuffer.put_color(self.footer, self.mode.footer_color());
        }

        {
            self.framebuffer.push_frame();
            if !CONF.retain_frame {
                self.framebuffer.clear();
            }
        }

        log(&format!("cursor: {:?}", self.fileview.cursor));
    }

    fn process_input(&mut self) {
        let c = read_input();

        log(&format!("input: {:?}", c));

        // TODO: more sophisticated cursor movement ...
        match c {
            Input::Key('h')    => self.mv_cursor(Move::Left),
            Input::Key('j')    => self.mv_cursor(Move::Down),
            Input::Key('k')    => self.mv_cursor(Move::Up),
            Input::Key('l')    => self.mv_cursor(Move::Right),
            Input::Key('\t')   => self.switch_mode(),
            Input::Key('\\')   => Debugconsole::clear(),
            Input::Key('d')    => self.filebuffer.line_del((self.framebuffer.cursor.y - 1) /* because not text coordinate */ as usize),
            Input::Key('u')    => self.filebuffer.undo(),
            Input::Key('s')    => self.filebuffer.to_file(&format!("{}.tmp", self.fileview.filepath)),

            Input::Key(CTRL_C) => self.running = false,

            // noop by default
            _       => (),
        }

        // TODO: update the fileview here regardless of the operation !
        //  for instance delete line can force a cursor update
        self.fileview.update(&self.filebuffer);
    }

    fn mv_cursor(&mut self, m : Move) {
        let delta = match m {
            Move::Left  => vek(-1,0),
            Move::Right => vek(1,0),
            Move::Up    => vek(0,-1),
            Move::Down  => vek(0,1),
            _           => vek(0,0),
        };

        // TODO: update the 'desired cursor position' instead of the real cursor position
        self.fileview.cursor = self.fileview.cursor + delta;
    }

    fn switch_mode(&mut self) {
        let newmode = match self.mode {
            EditorMode::Command =>  EditorMode::Insert,
            EditorMode::Insert  =>  EditorMode::Command,
        };
        self.mode = newmode;
    }

    fn resize(&mut self) {
        // TODO
    }
}







type Rez<T> = result::Result<T, String>;

// TODO: associate this to a Filebuffer struct
// TODO: probably I need to collapse all errors into strings, and create my own Result alias ...
fn file_load(filename: &str) -> io::Result<Vec<u8>> {
    let fileinfo = try!(fs::metadata(filename));
    let size = fileinfo.len() as usize;

    let mut buf = vec![0; size];
    let mut f = try!(fs::File::open(filename));

    let nread = try!(f.read(&mut buf));
    if nread != size {
        // why so ugly ...
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "not read enough bytes")); // TODO: add number of bytes
    }

    Ok(buf)
}

fn main() {
    let rez;

    {
        let term = Term::set_raw();

        let filename = file!();
        let buf = file_load(filename).unwrap();

        let filebuffer = Filebuffer::from_file(buf);
        //file_lines_print(&buf);

        rez = std::panic::catch_unwind(|| {
            Editor::mk_editor(filename.to_string(), filebuffer).run();
        });
    }

    rez.unwrap();
}





/* TERMINAL BINDINGS */

#[repr(C)]
struct TermWinsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

// BUG: this appears to not be correctly linked statically ...
#[link(name = "term", kind = "static")]
extern "C" {
    fn terminal_get_size() -> TermWinsize;
    fn terminal_restore();
    fn terminal_set_raw() -> i32;
}

// Empty object used to safely control terminal raw mode and properly exit raw mode at scope exit.
struct Term {
}

impl Term {
    fn size() -> Vek {
        unsafe {
            let ws = terminal_get_size();
            vek(ws.ws_col as i32, ws.ws_row as i32)
        }
    }

    fn set_raw() -> Term {
        if CONF.no_raw_mode {
            return Term { }
        }

        let stdout = io::stdout();
        let mut h = stdout.lock();
        h.write(term_cursor_save).unwrap();
        h.write(term_switch_offscreen).unwrap();
        h.write(term_switch_mouse_event_on).unwrap();
        h.write(term_switch_mouse_tracking_on).unwrap();
        h.flush().unwrap();

        unsafe {
            let _ = terminal_set_raw();
        }

        Term { }
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        if CONF.no_raw_mode {
            return
        }

        let stdout = io::stdout();
        let mut h = stdout.lock();
        h.write(term_switch_mouse_tracking_off).unwrap();
        h.write(term_switch_mouse_event_off).unwrap();
        h.write(term_switch_mainscreen).unwrap();
        h.write(term_cursor_restore).unwrap();
        h.flush().unwrap();

        unsafe {
            terminal_restore();
        }
    }
}


const term_start                      : &[u8] = b"\x1b[";
const term_finish                     : &[u8] = b"\x1b[0m";
const term_clear                      : &[u8] = b"\x1bc";
const term_cursor_hide                : &[u8] = b"\x1b[?25l";
const term_cursor_show                : &[u8] = b"\x1b[?25h";
const term_cursor_save                : &[u8] = b"\x1b[s";
const term_cursor_restore             : &[u8] = b"\x1b[u";
const term_switch_offscreen           : &[u8] = b"\x1b[?47h";
const term_switch_mainscreen          : &[u8] = b"\x1b[?47l";
const term_switch_mouse_event_on      : &[u8] = b"\x1b[?1000h";
const term_switch_mouse_tracking_on   : &[u8] = b"\x1b[?1002h";
const term_switch_mouse_tracking_off  : &[u8] = b"\x1b[?1002l";
const term_switch_mouse_event_off     : &[u8] = b"\x1b[?1000l";
const term_switch_focus_event_on      : &[u8] = b"\x1b[?1004h";
const term_switch_focus_event_off     : &[u8] = b"\x1b[?1004l";
const term_gohome                     : &[u8] = b"\x1b[H";
const term_newline                    : &[u8] = b"\r\n";



/* KEY INPUT HANDLING */

// TODO: pretty print control codes
#[derive(Debug, Clone, Copy)]
enum Input {
    Noinput,
    Key(char),
    Click(Vek),
    ClickRelease(Vek),
    UnknownEscSeq,
    EscZ,       // shift + tab -> "\x1b[Z"
    Resize,
    Error,
}

const NO_KEY    : char = 0 as char;
const CTRL_C    : char = 3 as char;
const CTRL_D    : char = 4 as char;
const CTRL_F    : char = 6 as char;
const CTRL_H    : char = 8 as char;
const TAB       : char = 9 as char;       // also ctrl + i
const RETURN    : char = 10 as char;      // also ctrl + j
const CTRL_K    : char = 11 as char;
const CTRL_L    : char = 12 as char;
const ENTER     : char = 13 as char;
const CTRL_Q    : char = 17 as char;
const CTRL_S    : char = 19 as char;
const CTRL_U    : char = 21 as char;
const CTRL_Z    : char = 26 as char;
const ESC       : char = 27 as char;      // also ctrl + [
const BACKSPACE : char = 127 as char;


fn is_printable(c : char) -> bool {
    ESC < c && c < BACKSPACE
}

fn read_char() -> char {
    let mut stdin = io::stdin();
    let mut buf = [0;1];
    // TODO: handle interrupts when errno == EINTR
    // TODO: propagate error otherwise
    // TODO: support unicode !
    loop {
        let n = stdin.read(&mut buf).unwrap();
        if n == 1 {
            break;
        }
        // otherwise, it's a timeout => retry
    }

    buf[0] as char
}

fn read_input() -> Input {
    let c = read_char();

    if c != ESC {
        return Input::Key(c);
    }

    // Escape sequence
    assert_eq!(read_char(), '[');

    // BUG: mouse clicks are not correctly parsed
    match read_char() {
        'M' =>  (), // Mouse click, handled below
        'Z' =>  return Input::EscZ,
        _   =>  return Input::UnknownEscSeq
    }

    // Mouse click
    // TODO: support other mouse modes
    let c2 = read_char();
    let mut x = (read_char() as i32) - 33;
    let mut y = (read_char() as i32) - 33;
    if x < 0 {
        x += 255;
    }
    if y < 0 {
        y += 255;
    }

    let v = vek(x,y);

    // BUG: does not work currently. Maybe terminal setup is wrong ?
    match c2 as i32 {
        0 ... 2 =>  return Input::Click(v),
        3       =>  return Input::ClickRelease(v),
        _       =>  return Input::UnknownEscSeq,
    }
}
