#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_macros)]


use std::cmp::max;
use std::cmp::min;
use std::error::Error;
use std::fmt::Display;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem::replace;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc;
use std::thread;


macro_rules! check {
    ($test:expr, $cause:expr) => {
        assert!($test, format!("{}:{} cause: {}", file!(), line!(), $cause))
    };
    ($test:expr) => {
        check!($test, "unknown")
    };
}

macro_rules! er {
    ($cause: expr) => {
        Err(Er { descr: format!("{}:{} cause: {}", file!(), line!(), $cause) })
    };
}


/*
 * Next Steps:
 *  - text insert:
 *      - line copy, line break,
 *      - add insert at x offset
 *      - add replace at x offset
 *  - redo
 *  - cursor horizontal memory
 *  - better navigation
 *
 * General TODOs:
 *  - handle resize
 *  - dir explorer
 *  - think more about where to track the screen area:
 *      right now it is repeated both in Screen and in View
 *      ideally Screen would not be tracking it
 *  - utf8 support: Line and Filebuffer, Input, ... don't wait too much
 */



// Global constant that controls a bunch of options.
const CONF : Config = Config {
    draw_screen:            true,
    draw_colors:            true,
    retain_frame:           false,
    no_raw_mode:            false, //true,

    debug_console:          true,
    debug_bounds:           true,
    debug_latency:          true,

    relative_lineno:        true,
    cursor_show_line:       true,
    cursor_show_column:     true,

    color_default:          Colorcell { fg: Color::Black,   bg: Color::White },
    color_header_active:    Colorcell { fg: Color::Gray(2), bg: Color::Yellow },
    color_header_inactive:  Colorcell { fg: Color::Gray(2), bg: Color::Cyan },
    color_footer:           Colorcell { fg: Color::White,   bg: Color::Gray(2) },
    color_lineno:           Colorcell { fg: Color::Green,   bg: Color::Gray(2) },
    color_console:          Colorcell { fg: Color::White,   bg: Color::Gray(16) },
    color_cursor_lines:     Colorcell { fg: Color::Black,   bg: Color::Gray(6) },

    color_mode_command:     Colorcell { fg: Color::Gray(1), bg: Color::Black },
    color_mode_insert:      Colorcell { fg: Color::Gray(1), bg: Color::Red },
    color_mode_exit:        Colorcell { fg: Color::Magenta, bg: Color::Magenta },
};


// TODO: experiment with a static framebuffer that has a &mut[u8] instead of a vec.


/* CORE TYPE DEFINITION */

// The core editor structure
struct Editor {
    window:         Vek,              // The dimensions of the editor and backend terminal window
    mainscreen:     Rec,              // The screen area for displaying file content and menus.
    footer:         Rec,
    framebuffer:    Framebuffer,

    // TODO:
    //  list of open files and their filebuffers
    //  list of screens
    //  current screen layout
    //  Mode state machine

    // For the time being, only one file can be loaded and edited per program.
    buffer: Buffer,
    view:   View,
}


struct Config {
    draw_screen:            bool,
    draw_colors:            bool,
    retain_frame:           bool,
    no_raw_mode:            bool,

    debug_console:          bool,
    debug_bounds:           bool,
    debug_latency:          bool,

    relative_lineno:        bool,
    cursor_show_line:       bool,
    cursor_show_column:     bool,

    color_default:          Colorcell,
    color_header_active:    Colorcell,
    color_header_inactive:  Colorcell,
    color_footer:           Colorcell,
    color_lineno:           Colorcell,
    color_console:          Colorcell,
    color_cursor_lines:     Colorcell,

    color_mode_command:     Colorcell,
    color_mode_insert:      Colorcell,
    color_mode_exit:        Colorcell,
}

// Either a position in 2d space w.r.t to (0,0), or a movement quantity
#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Command(Commandstate),
    Insert(Insertstate),
    PendingInsert,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Commandstate {
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Insertstate {
}

// Left justified, fixed length strings.
const MODE_COMMAND : &'static str = "Command";
const MODE_INSERT  : &'static str = "Insert ";
const MODE_PINSERT : &'static str = "Insert?";
const MODE_EXIT    : &'static str = "Exit   ";

impl Mode {
    const default_command_state : Mode = Mode::Command(Commandstate { });

    fn footer_color(self) -> Colorcell {
        use Mode::*;
        match self {
            Command(_)      => CONF.color_mode_command,
            Insert(_)       => CONF.color_mode_insert,
            PendingInsert   => CONF.color_mode_insert,
            Exit            => CONF.color_mode_exit,
        }
    }

    fn name(self) -> &'static str {
        use Mode::*;
        match self {
            Command(_)      => MODE_COMMAND,
            Insert(_)       => MODE_INSERT,
            PendingInsert   => MODE_PINSERT,
            Exit            => MODE_EXIT,
        }
    }

    fn process_input(m: Mode, i: Input, e: &mut Editor) -> Re<Mode> {
        if i == Input::Key(CTRL_C) {
            return Ok(Exit)
        }

        if i == Input::Resize {
            log("resize !");
            return Ok(m)
        }

        use Mode::*;
        let next = match m {
            Command(mut state) => {
                let op = Mode::input_to_command_op(i, e);
                state.do_command(op, e)?
            }

            Insert(mut state) => {
                let op = Mode::input_to_insert_op(i, e);
                state.do_insert(op, e)?
            }

            PendingInsert => {
                e.buffer.snapshot();
                let insertmode = Insert(e.buffer.start_insert(e.view.cursor));
                Mode::process_input(insertmode, i, e)?
            }

            Exit => {
                panic!("cannot process input in Exit state")
            }
        };

        // TODO: instead of aborting, handle input error processing, and check if any dirty files
        // need saving !

        // TODO: when should views be updated exactly ?
        //       probably on any buffer mutation and every cursor movement
        e.view.update(&e.buffer);

        Ok(next)
    }

    fn input_to_command_op(i: Input, e: &Editor) -> CommandOp {
        use Input::*;
        use CommandOp::*;
        match i {
            // TODO: more sophisticated cursor movement ...
            Key('h')    => Movement(Move::Left),
            Key('j')    => Movement(Move::Down),
            Key('k')    => Movement(Move::Up),
            Key('l')    => Movement(Move::Right),
            // TODO: Consider changing to Mut(LineOp, cursor) for something more systematic ?
            Key('o')    => LineNew(e.view.cursor),
            Key(ENTER)  => LineBreak(e.view.cursor),
            Key('d')    => LineDel(e.view.cursor),
            Key('u')    => Undo,
            Key('U')    => Redo,
            Key('\t')   => SwitchInsert,
            Key('s')    => Save(format!("{}.tmp", e.view.filepath)),
            Key('\\')   => ClearConsole,
            Key('b')
                        => panic!("BOOM !"),
                        //=> return er!("BOOM !"),
            // TODO: handle mouse click
            _ => Noop,
        }
    }

    fn input_to_insert_op(i: Input, e: &Editor) -> InsertOp {
        use Input::*;
        use InsertOp::*;
        match i {
            Key(ESC) | EscZ => SwitchCommand,
            Key(ENTER)      => BreakLine(e.view.cursor),
            Key(c)          => CharInsert(e.view.cursor, c),
            _ => Noop,
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
    cursor:     Vek,            // Absolute screen coordinate relative to (0,0).

    buffer:     Vec<u8>,
}


// Transient object for putting text into a subrectangle of a framebuffer.
// All positions are w.r.t the Framebuffer (0,0) origin.
// Since it needs a mut ref to the framebuffer, Screen objs cannot be stored.
struct Screen<'a> {
    framebuffer:    &'a mut Framebuffer,
    window:         Rec,
    linenoarea:     Rec,
    textarea:       Rec,
    header:         Rec,
    view:       &'a View,
    // TODO: consider adding Buffer directly here too
}

enum Draw {
    Nothing,
    All,
    Header,
    Text,
}

struct Textbuffer {
    text:   Vec<u8>,    // original content of the file when loaded
    lines:  Vec<Line>,  // subslices into the buffer or appendbuffer
}

#[derive(Clone)] // Do I need clone ??
struct Textsnapshot {
    line_indexes: Vec<usize> // the actual lines in the current files, as indexes into 'lines'
}

// Manage content of a file
// Q: can I have a vec in a struct and another subslice pointing into that vec ?
//    I would need to say that they both have the same lifetime as the struct.
struct Buffer {
    textbuffer:             Textbuffer,
    previous_snapshots:     Vec<Textsnapshot>,
    current_snapshot:       Textsnapshot,
}

// A pair of offsets into a buffer for delimiting lines.
#[derive(Debug, Clone, Copy)]
struct Line {
    start:  usize,      // inclusive
    stop:   usize,      // exclusive
}

fn line(start: usize, stop: usize) -> Line {
    check!(start <= stop);
    Line { start , stop }
}

enum CommandOp {
    LineDel(Vek),
    LineNew(Vek),
    LineBreak(Vek),
    Movement(Move),
    Undo,
    Redo,
    Save(String),
    SwitchInsert,
    ClearConsole,
    Noop,
}

enum InsertOp {
    BreakLine(Vek),
    CharInsert(Vek, char),
    SwitchCommand,
    Noop,
}


// Point to a place inside a Buffer
struct Cursor<'a> {
    buffer: &'a Buffer,
}

// Store states related to navigation in a given file.
// All positions are in text coordinate.
struct View {
    filepath:           String,
    relative_lineno:    bool,
    movement_mode:      MovementMode,
    show_token:         bool,
    show_neighbor:      bool,
    show_selection:     bool,
    is_active:          bool,
    cursor:             Vek,
    cursor_memory:      Vek,
    filearea:           Rec,
    //selection:  Option<&[Selection]>
}

impl View {
    fn mk_fileview(filepath: String, screensize: Vek) -> View {
        View {
            filepath,
            relative_lineno:    CONF.relative_lineno,
            movement_mode:      MovementMode::Chars,
            show_token:         false,
            show_neighbor:      false,
            show_selection:     false,
            is_active:          true,
            cursor:             vek(0,0),
            cursor_memory:      vek(0,0),
            filearea:           screensize.rec(),
        }
    }

    fn update(&mut self, buffer: &Buffer) {
        if buffer.nlines() == 0 {
            return;
        }

        // Cursor adjustment
        {
            let mut p = self.cursor;

            p.y = min(p.y, buffer.nlines() - 1);
            p.y = max(0, p.y);

            // Right bound clamp pushes x to -1 for empty lines.
            p.x = min(p.x, i32(buffer.line_len(usize(p.y))) - 1);
            p.x = max(0, p.x);

            self.cursor = p;
        }

        // text range adjustment
        {
            let p = self.cursor;

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
        }
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
        check!(self.min.y <= y, "row was out of bounds (left)");
        check!(y <= self.max.y, "row was out of bounds (right)");
        rec(self.min.x, y, self.max.x, y + 1)
    }

    fn column(self, x: i32) -> Rec {
        check!(self.min.x <= x, "column was out of bounds (top)");
        check!(x <= self.max.x, "column was out of bounds (bottom)");
        rec(x, self.min.y, x + 1, self.max.y)
    }

    // TODO: should x be forbidden from matching the bounds (i.e no empty output)
    fn hsplit(self, x: i32) -> (Rec, Rec) {
        check!(self.min.x <= x);
        check!(x < self.max.x);

        let left = rec(self.min.x, self.min.y, x, self.max.y);
        let right = rec(x, self.min.y, self.max.x, self.max.y);

        (left, right)
    }

    fn vsplit(self, y: i32) -> (Rec, Rec) {
        check!(self.min.y <= y);
        check!(y < self.max.y);

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

impl Display for Vek {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
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
    use Color::*;
    match c {
        // TODO !
        Black                    => 0,
        Red                      => 1,
        Green                    => 2,
        Yellow                   => 3,
        Blue                     => 4,
        Magenta                  => 5,
        Cyan                     => 6,
        White                    => 7,
        BoldBlack                => 8,
        BoldRed                  => 9,
        BoldGreen                => 10,
        BoldYellow               => 11,
        BoldBlue                 => 12,
        BoldMagenta              => 13,
        BoldCyan                 => 14,
        BoldWhite                => 15,
        RGB216 { r, g, b }       => 15 + (r + 6 * (g + 6 * b)),
        Gray(g)                  => 255 - g,
    }
}


/* UTILITIES */

type Re<T> = Result<T, Er>;

#[derive(Debug)]
struct Er {
    descr: String,
}

impl Display for Er {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.descr)
    }
}

impl std::error::Error for Er {
    fn description(&self) -> &str {
        &self.descr
    }
}

impl From<io::Error> for Er {
    fn from(err: io::Error) -> Er {
        Er { descr: format!("{}:?? cause: {}", file!(), err.description()) }
    }
}


impl From<mpsc::RecvError> for Er {
    fn from(err: mpsc::RecvError) -> Er {
        Er { descr: format!("{}:?? cause: {}", file!(), err.description()) }
    }
}

struct Scopeclock<'a> {
    tag: &'a str,
    timestamp: std::time::SystemTime,
}

impl <'a> Scopeclock<'a> {
    fn measure(tag: &'a str) -> Scopeclock {
        let timestamp = std::time::SystemTime::now();

        Scopeclock { tag, timestamp }
    }
}

const zero_duration : std::time::Duration = std::time::Duration::from_millis(0);

impl <'a> Drop for Scopeclock<'a> {
    fn drop(&mut self) {
        if !CONF.debug_latency {
            return
        }
        let dt = self.timestamp.elapsed().unwrap_or(zero_duration);
        log(&format!("{}: {}.{:06}", self.tag, dt.as_secs(), dt.subsec_nanos() / 1000));
    }
}


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

fn i32(x: usize) -> i32 {
    x as i32
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
            buffer:     vec![0; 64 * 1024],
        }
    }

    // TODO: add clear in sub rec
    fn clear(&mut self) {
        fill(&mut self.text, frame_default_text);
        fill(&mut self.fg,   frame_default_fg);
        fill(&mut self.bg,   frame_default_bg);
    }

    fn put_line(&mut self, pos: Vek, src: &[u8]) {
        check!(self.window.rec().contains(pos));

        let maxlen = (self.window.x - pos.x) as usize;
        let len = min(src.len(), maxlen);

        let start = (pos.y * self.window.x + pos.x) as usize;
        let stop = start + len;

        copy_exact(&mut self.text[start..stop], &src[..len]);
    }

    // area.min is inclusive, area.max is exclusive
    fn put_color(&mut self, area: Rec, colors: Colorcell) {
        if CONF.debug_bounds {
            check!(0 <= area.x0());
            check!(0 <= area.y0());
            check!(area.x1() <= self.window.x);
            check!(area.y1() <= self.window.y);
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
    fn render(&mut self) -> Re<()> {
        if !CONF.draw_screen {
            return Ok(())
        }

        unsafe {
            CONSOLE.write_into(self);
        }

        fn append(dst: &mut Vec<u8>, src: &[u8]) {
            dst.extend_from_slice(src);
        }

        let mut buffer = replace(&mut self.buffer, Vec::new());
        unsafe {
            buffer.set_len(0); // safe because element are pure values
        }

        append(&mut buffer, term_cursor_hide);
        append(&mut buffer, term_gohome);

        let w = self.window.x as usize;
        let mut l = 0;
        let mut r = w;
        for i in 0..self.window.y {
            if i > 0 {
                // Do not put "\r\n" on the last line
                append(&mut buffer, term_newline);
            }

            if CONF.draw_colors {
                let mut j = l;
                loop {
                    let k = self.find_color_end(j, r);

                    // PERF: better color command creation without multiple string allocs.
                    let fg_code = colorcode(self.fg[j]);
                    let bg_code = colorcode(self.bg[j]);
                    append(&mut buffer, format!("\x1b[38;5;{};48;5;{}m", fg_code, bg_code).as_bytes());
                    append(&mut buffer, &self.text[j..k]);
                    if k == r {
                        break;
                    }
                    j = k;
                }
            } else {
                append(&mut buffer, &self.text[l..r]);
            }

            l += w;
            r += w;
        }

        // Terminal cursor coodinates start at (1,1)
        let cursor_command = format!("\x1b[{};{}H", self.cursor.y + 1, self.cursor.x + 1);
        append(&mut buffer, cursor_command.as_bytes());
        append(&mut buffer, term_cursor_show);

        // IO to terminal
        {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let n = handle.write(&buffer)?;
            handle.flush()?;

            check!(n == buffer.len());
        }

        // Put back buffer in place for reuse
        self.buffer = buffer;

        Ok(())
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
    fn mk_screen<'b>(window: Rec, framebuffer: &'b mut Framebuffer, view: &'b View) -> Screen<'b> {
        let lineno_len = 5;
        let (header, filearea) = window.vsplit(1);
        let (linenoarea, textarea) = filearea.hsplit(lineno_len);

        Screen {
            framebuffer,
            window,
            linenoarea,
            textarea,
            header,
            view,
        }
    }

    fn draw(&mut self, draw: Draw, buffer: &Buffer) {
        // TODO: use draw and only redraw what's needed
        // TODO: automatize the screen coordinate offsetting for framebuffer commands
        let file_base_offset = self.view.filearea.min;
        let frame_base_offset = self.textarea.min;

        // header
        {
                let header_string = format!("{}  {:?}", self.view.filepath, self.view.movement_mode);
                self.framebuffer.put_line(self.header.min, header_string.as_bytes());
                self.framebuffer.put_color(self.header, CONF.color_header_active);
        }

        // buffer content
        {
            let y_stop = min(self.textarea.h(), buffer.nlines() - file_base_offset.y);
            for i in 0..y_stop {
                let lineoffset = vek(0, i);
                let file_offset = file_base_offset + lineoffset;
                let frame_offset = frame_base_offset + lineoffset;

                let mut line = buffer.line_get_slice(file_offset);
                line = clamp(line, self.textarea.w() as usize);
                self.framebuffer.put_line(frame_offset, line);
            }
        }

        // lineno
        {
            let mut buf = [0 as u8; 4];
            let lineno_base = if self.view.relative_lineno {
                file_base_offset.y - self.view.cursor.y
            } else {
                file_base_offset.y + 1
            };
            for i in 0..self.textarea.h() {
                itoa10(&mut buf, lineno_base + i, ' ' as u8);
                self.framebuffer.put_line(self.linenoarea.min + vek(0,i), &buf);
            }
            self.framebuffer.put_color(self.linenoarea, CONF.color_lineno);
        }

        {
            let cursor_screen_position = self.view.cursor + self.textarea.min - file_base_offset;
            if self.view.is_active {
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
        self.stop - self.start
    }

    fn cut(self, n: usize) -> (Line, Line) {
        let pivot = self.start + n;
        check!(pivot <= self.stop);
        (line(self.start, pivot), line(pivot, self.stop))
    }
}

impl Textbuffer {
    fn append(&mut self, c: char) {
        self.text.push(c as u8);
        self.lines.last_mut().unwrap().stop += 1;
    }

    fn insert(&mut self, colno: usize, c: char) {
        let (left, right) = self.lines.last().unwrap().cut(colno);
        self.append(' ');

        for i in (right.start..right.stop).rev() {
            self.text[i+1] = self.text[i];
        }
        self.text[right.start] = c as u8;
    }

    // TODO: replace char mode

    fn emptyline(&mut self) -> usize {
        self.lines.push(line(self.text.len(), self.text.len()));

        self.lines.len() - 1
    }

    fn copyline(&mut self, line_idx: usize) -> usize {
        let src = self.lines[line_idx];
        let dststart = self.text.len();
        let dststop = dststart + src.len();
        let dst = line(dststart, dststop);

        self.text.reserve(src.len());
        for i in src.start..src.stop {
            // This is so lame ;-( I want my memcpy
            let c = self.text[i];
            self.text.push(c);
        }

        self.lines.push(dst);
        self.lines.len() - 1
    }
}

impl Buffer {
    fn from_file(path: &str) -> Re<Buffer> {
        let text = file_load(path)?;

        Ok(Buffer::from_text(text))
    }

    fn from_text(text: Vec<u8>) -> Buffer {

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
                lines.push(line(a, b));
                a = b + 1; // skip the '\n'

                if lines.len() == 20 {
                    break;
                }
            }
        }

        for i in 0..lines.len() {
            line_indexes.push(i);
        }

        // HACK: just temporary until I had cursor position to append and have proper insert mode !
        //       this is necessary to start a newline for appending chars, until command -> insert
        //       mode transation does this properly.
        {
            line_indexes.push(lines.len());
            lines.push(line(text.len(), text.len()));
        }

        Buffer {
            textbuffer:         Textbuffer { text, lines },
            previous_snapshots: Vec::new(),
            current_snapshot:   Textsnapshot { line_indexes },
        }
    }

    // TODO: propagate errors
    fn to_file(&self, path: &str) -> Re<()> {
        let mut f = File::create(path)?;

        for i in 0..self.nlines() {
            f.write_all(self.line_get_slice(vek(0,i)))?;
            f.write_all(b"\n")?; // TODO: use platform's newline
        }

        Ok(())
    }

    fn snapshot(&mut self) {
        let next_snapshot = self.current_snapshot.clone();
        let prev_snapshot = replace(&mut self.current_snapshot, next_snapshot);
        self.previous_snapshots.push(prev_snapshot);
    }

    fn nlines(&self) -> i32 {
        i32(self.current_snapshot.line_indexes.len())
    }

    fn line_len(&self, y: usize) -> usize {
        let idx = self.current_snapshot.line_indexes[y as usize];
        self.textbuffer.lines[idx].len()
    }

    fn line_index(&self, lineno: usize) -> usize {
        self.current_snapshot.line_indexes[lineno]
    }

    fn line_get(&self, lineno: usize) -> Line {
        self.textbuffer.lines[self.line_index(lineno)]
    }

    fn line_get_slice<'a>(&'a self, offset: Vek) -> &'a[u8] {
        let x = offset.x as usize;
        let y = offset.y as usize;
        let line = self.line_get(y).to_slice(&self.textbuffer.text);
        shift(line, x)
    }

    fn line_del(&mut self, y: usize) {
        check!(y < self.current_snapshot.line_indexes.len());
        self.snapshot();
        self.current_snapshot.line_indexes.remove(y);

        // TODO: update all other views of that file whose cursors is below lineno
    }

    fn line_new(&mut self, lineno: usize) {
        let lastline = self.current_snapshot.line_indexes.len() - 1;
        self.current_snapshot.line_indexes.reserve(1);
        for i in (lineno..lastline).rev() {
            self.current_snapshot.line_indexes[i+1] = self.current_snapshot.line_indexes[i];
        }

        self.current_snapshot.line_indexes[lineno] = self.textbuffer.emptyline();

        // TODO: update all other views of that file whose cursors is below lineno
    }

    fn line_break(&mut self, lineno: usize, colno: usize) {
        let (left, right) = self.line_get(lineno).cut(colno);

        self.line_new(lineno);
        let left_idx = self.line_index(lineno);
        let right_idx = self.line_index(lineno + 1);
        self.textbuffer.lines[left_idx]  = left;
        self.textbuffer.lines[right_idx] = right;

        // TODO: update all other views of that file whose cursors is below lineno
    }

    fn undo(&mut self) {
        match self.previous_snapshots.pop() {
            Some(prev_snapshot) => {
                self.current_snapshot = prev_snapshot
            }
            None => (),
        }
    }

    fn insert(&mut self, cursor: Vek, c: char) {
        let lineno = usize(cursor.y);
        let colno = usize(cursor.x);

        let last_idx = self.textbuffer.lines.len() - 1;
        let line_idx = self.line_index(lineno);
        if line_idx != last_idx {
            let newline_idx = self.textbuffer.copyline(line_idx);
            self.current_snapshot.line_indexes[lineno] = newline_idx;
        }

        self.textbuffer.insert(colno, c);
    }

    fn start_insert(&mut self, cursor: Vek) -> Insertstate {
        // TODO !
        Insertstate { }
    }

    fn stop_insert(&mut self, cursor: Vek) {
        // TODO: do I need this ?
    }

    // TODO: append mode, replace mode
}

impl Commandstate {
    fn do_command(&mut self, op: CommandOp, e: &mut Editor) -> Re<Mode> {
        use CommandOp::*;
        match op {
            Movement(mvt)   => e.mv_cursor(mvt),

            LineDel(pos) => {
                if e.buffer.nlines() > 0 {
                    let lineno = usize(pos.y);
                    e.buffer.snapshot();
                    e.buffer.line_del(lineno);
                }
            }

            LineNew(pos) => {
                let lineno = usize(pos.y);
                e.buffer.snapshot();
                e.buffer.line_new(lineno);
            }

            LineBreak(pos) => {
                let lineno = usize(pos.y);
                let colno = usize(pos.x);
                e.buffer.snapshot();
                e.buffer.line_break(lineno, colno);
            }

            Undo => e.buffer.undo(), // TODO: implement redo stack

            Redo => (), // TODO !

            Save(path) => e.buffer.to_file(&path)?,

            ClearConsole => Debugconsole::clear(),

            SwitchInsert => return Ok(Mode::PendingInsert),

            Noop => (),
        }

        Ok(Mode::Command(replace(self, Commandstate { })))
    }
}

impl Insertstate {
    fn do_insert(&mut self, op: InsertOp, e: &mut Editor) -> Re<Mode> {
        use InsertOp::*;
        match op {
            BreakLine(cursor) => {
                // TODO: take into account cursor !
                let newline = e.buffer.textbuffer.emptyline();
                e.buffer.current_snapshot.line_indexes.push(newline)
            }

            CharInsert(cursor, c) => {
                if is_printable(c) {
                    // TODO: take into account cursor
                    e.buffer.insert(cursor, c);
                    // TODO update cursor right there ?
                    // TODO: think abotu auto linebreak
                }
            }

            SwitchCommand =>
                return Ok(Mode::default_command_state),

            Noop => (),
        }

        Ok(Mode::Insert(replace(self, Insertstate { })))
    }
}

impl Editor {

    fn mk_editor() -> Re<Editor> {
        let filename = file!().to_string();
        let buffer = Buffer::from_file(&filename)?;

        let window = Term::size();
        let framebuffer = Framebuffer::mk_framebuffer(window);
        let (mainscreen, footer) = window.rec().vsplit(window.y - 1);
        let view;
        {
            // reuse code in mk_Screen !!!
            let (_, filearea) = mainscreen.vsplit(1);
            let (_, textarea) = filearea.hsplit(5);
            view = View::mk_fileview(filename, textarea.size());
        }

        Ok(Editor {
            window,
            mainscreen,
            footer,
            framebuffer,
            buffer,
            view,
        })
    }

    fn run() -> Re<()> {
        let mut e = Editor::mk_editor()?;
        let mut m = Mode::default_command_state;

        e.refresh_screen(&m)?;

        let (send, recv) = mpsc::sync_channel(32);

        thread::spawn(move || {
            push_char(&send);
        });


        while m != Mode::Exit {
            let i = pull_input(&recv)?;
            log(&format!("input: {:?}", i));

            let frame_time = Scopeclock::measure("last frame");     // caveat: displayed on next frame only

            m = Mode::process_input(m, i, &mut e)?;

            // CLEANUP: extract Framebuffer from screen and do framebuffer:refresh(&e, &m):
            e.refresh_screen(&m)?;
        }

        Ok(())
    }

    fn refresh_screen(&mut self, mode: &Mode) -> Re<()> {
        // main screen
        {
            let draw_time = Scopeclock::measure("draw");

            let mut screen = Screen::mk_screen(self.mainscreen, &mut self.framebuffer, &self.view);
            screen.draw(Draw::All, &self.buffer);
        }

        // footer
        {
            self.framebuffer.put_line(self.footer.min + vek(1,0), mode.name().as_bytes());
            self.framebuffer.put_line(self.footer.min + vek(10, 0), b"FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER");
            self.framebuffer.put_color(self.footer, mode.footer_color());
        }

        // renter frame to terminal
        {
            let push_frame_time = Scopeclock::measure("render");

            self.framebuffer.render()?;
            if !CONF.retain_frame {
                self.framebuffer.clear();
            }
        }

        Ok(())
    }

    fn mv_cursor(&mut self, m : Move) {
        use Move::*;
        let delta = match m {
            Left  => vek(-1,0),
            Right => vek(1,0),
            Up    => vek(0,-1),
            Down  => vek(0,1),
            _     => vek(0,0),
        };

        // TODO: update the 'desired cursor position' instead of the real cursor position
        self.view.cursor = self.view.cursor + delta;
    }

    fn resize(&mut self) {
        // TODO
    }
}







// TODO: associate this to a Buffer struct
// TODO: probably I need to collapse all errors into strings, and create my own Result alias ...
fn file_load(filename: &str) -> Re<Vec<u8>> {
    let fileinfo = fs::metadata(filename)?;
    let size = fileinfo.len() as usize;

    let mut buf = vec![0; size];
    let mut f = File::open(filename)?;

    let nread = f.read(&mut buf)?;
    if nread != size {
        return er!("not enough bytes");
    }

    Ok(buf)
}


fn main() {
    let term = Term::set_raw().unwrap();

    Editor::run().unwrap();
}


/* TERMINAL BINDINGS */

#[repr(C)]
struct TermWinsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[link(name = "term", kind = "static")]
extern "C" {
    fn terminal_get_size() -> TermWinsize;
    fn terminal_restore();
    fn terminal_set_raw() -> i32;
    fn read_1B() -> i32;
}

// Global variable for ensuring terminal restore happens once exactly.
static mut is_raw : bool = false;

// Empty object used to safely control terminal raw mode and properly exit raw mode at scope exit.
struct Term {
}

impl Drop for Term {
    fn drop(&mut self) {
        Term::restore();
    }
}

impl Term {
    fn size() -> Vek {
        unsafe {
            let ws = terminal_get_size();
            vek(ws.ws_col as i32, ws.ws_row as i32)
        }
    }

    fn set_raw() -> Re<Term> {
        if !CONF.no_raw_mode {
            let stdout = io::stdout();
            let mut h = stdout.lock();
            h.write(term_cursor_save)?;
            h.write(term_switch_offscreen)?;
            h.write(term_switch_mouse_event_on)?;
            h.write(term_switch_mouse_tracking_on)?;
            h.flush()?;

            unsafe {
                let _ = terminal_set_raw();
                is_raw = true;
            }

            // Ensure terminal is restored to default whenever a panic happens.
            let std_panic_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panicinfo| {
                Term::restore();
                std_panic_hook(panicinfo);
            }));
        }

        Ok(Term { })
    }

    fn restore() {
        unsafe {
            if !is_raw {
                return
            }
        }
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
            is_raw = false;
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
const CTRL_C    : char = 3 as char;       // end of text
const CTRL_D    : char = 4 as char;       // end of transmission
const CTRL_F    : char = 6 as char;
const CTRL_H    : char = 8 as char;
const TAB       : char = 9 as char;       // also ctrl + i
const LINE_FEED : char = 10 as char;      // also ctrl + j
const VTAB      : char = 11 as char;      // also ctrl + k
const NEW_PAGE  : char = 12 as char;      // also ctrl + l
const ENTER     : char = 13 as char;
const CTRL_Q    : char = 17 as char;
const CTRL_S    : char = 19 as char;
const CTRL_U    : char = 21 as char;
const CTRL_Z    : char = 26 as char;
const ESC       : char = 27 as char;      // also ctrl + [
const BACKSPACE : char = 127 as char;

const RESIZE    : char = 255 as char; //'\xff';


fn is_printable(c : char) -> bool {
    ESC < c && c < BACKSPACE
}

//fn read_char() -> Re<char> {
//    let c;
//    unsafe {
//        c = read_1B();
//    }
//    if c < 0 {
//        return er!(format!("error reading char ! errno={}", -c));
//    }
//
//    Ok(c as u8 as char)
//}


fn push_char(chan: &SyncSender<char>) {
    let mut stdin = io::stdin();
    let mut buf = [0;1];
    // TODO: handle interrupts when errno == EINTR
    // TODO: support unicode !
    loop {
        let n = stdin.read(&mut buf).unwrap(); // TODO: pass error through the channel ?
        if n == 1 {
            chan.send(buf[0] as char).unwrap();
        }
    }
}

fn pull_input(chan: &Receiver<char>) -> Re<Input> {
    use Input::*;
    //use std::sync::mpsc::TryRecvError;
    use std::sync::mpsc::TryRecvError::*;

    let c = chan.recv()?;

    if c == RESIZE {
        return Ok(Resize);
    }

    if c != ESC {
        return Ok(Key(c))
    }

    // Escape: if no more char immediately available, return ESC, otherwise parse an escape sequence

    match chan.try_recv() {
        Ok(c)       =>  check!(c == '['),   // This was an escape sequence, continue parsing
        Err(Empty)  => return Ok(Key(ESC)), // Nothing to read: this was an escape
        Err(e)      => return er!(e.description()),
    }

    match chan.recv()? {
        'M' =>  (), // Mouse click, handled below
        'Z' =>  return Ok(EscZ),
        _   =>  return Ok(UnknownEscSeq),
    }

    // Mouse click
    // TODO: support other mouse modes
    let c2 = chan.recv()? as i32;
    let mut x = chan.recv()? as i32 - 33;
    let mut y = chan.recv()? as i32 - 33;
    if x < 0 {
        x += 255;
    }
    if y < 0 {
        y += 255;
    }

    let v = vek(x,y);

    let r = match c2 & 3 /* ignore modifier keys */ {
        0 ... 2 =>  Click(v),
        3       =>  ClickRelease(v),
        _       =>  UnknownEscSeq,
    };

    Ok(r)
}
