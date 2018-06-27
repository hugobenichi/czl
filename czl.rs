#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_macros)]


use std::cmp::max;
use std::cmp::min;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::mem::replace;
use std::sync::mpsc;

use conf::*;
use core::*;
use draw::*;
use term::*;
use text::*;
use util::*;


macro_rules! check {
    ($test:expr, $cause:expr) => {
        assert!($test, format!("{}:{} cause: {}", file!(), line!(), $cause))
    };
    ($test:expr) => {
        check!($test, "unknown")
    };
}


/*
 * Features:
 *  - delete and backspace in command mode
 *  - offer to save if panic
 *  - better navigation !
 *  - copy and yank buffer
 *  - redo
 *  - cursor horizontal memory
 *  - buffer explorer
 *  - directory explorer
 *  - grep move
 *  - cursor previous points and cursor markers
 *  - ctags support
 *
 * TODOs and cleanups
 *  - need to implement char/line next/previous
 *  - BUG: double check Buffer::delete
 *  - BUG: why is there an empty last line at the end !!
 *  - migrate text snapshot to command list
 *  - fuzzer
 *  - handle resize
 *  - utf8 support: Range and Filebuffer, Input, ... don't wait too much
 */


fn main() {
    let term = Term::set_raw().unwrap();

    Editor::run().unwrap();
}


mod conf {


use core::*;


// Global constant that controls a bunch of options.
pub const CONF : Config = Config {
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

    // CLEANUP: use colorcell
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

    logfile:                &"/tmp/czl.log",
};


pub struct Config {
    pub draw_screen:            bool,
    pub draw_colors:            bool,
    pub retain_frame:           bool,
    pub no_raw_mode:            bool,

    pub debug_console:          bool,
    pub debug_bounds:           bool,
    pub debug_latency:          bool,

    pub relative_lineno:        bool,
    pub cursor_show_line:       bool,
    pub cursor_show_column:     bool,

    pub color_default:          Colorcell,
    pub color_header_active:    Colorcell,
    pub color_header_inactive:  Colorcell,
    pub color_footer:           Colorcell,
    pub color_lineno:           Colorcell,
    pub color_console:          Colorcell,
    pub color_cursor_lines:     Colorcell,

    pub color_mode_command:     Colorcell,
    pub color_mode_insert:      Colorcell,
    pub color_mode_exit:        Colorcell,

    pub logfile:                &'static str,
}


} // mod conf


// TODO: experiment with a static framebuffer that has a &mut[u8] instead of a vec.


/* CORE TYPE DEFINITION */

// The core editor structure
struct Editor {
    window:         Pos,              // The dimensions of the editor and backend terminal window
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
    PendingInsert(InsertMode),
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Commandstate {
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Insertstate {
    mode: InsertMode,
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
            Command(_)          => CONF.color_mode_command,
            Insert(_)           => CONF.color_mode_insert,
            PendingInsert(_)    => CONF.color_mode_insert,
            Exit                => CONF.color_mode_exit,
        }
    }

    fn name(self) -> &'static str {
        use Mode::*;
        match self {
            Command(_)          => MODE_COMMAND,
            Insert(_)           => MODE_INSERT,
            PendingInsert(_)    => MODE_PINSERT,
            Exit                => MODE_EXIT,
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
                let next = state.do_command(op, e)?;
                // should this instead be managed per operation in a more scoped way ?
                e.view.update(&e.buffer);
                next
            }

            Insert(mut state) => {
                let op = Mode::input_to_insert_op(i, e);
                state.do_insert(op, e)?
            }

            PendingInsert(mode) => {
                e.buffer.snapshot(e.view.cursor);
                let insertmode = Insert(Insertstate { mode });
                Mode::process_input(insertmode, i, e)?
            }

            Exit => {
                panic!("cannot process input in Exit state")
            }
        };

        // TODO: instead of aborting, handle input error processing, and check if any dirty files
        // need saving !

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
            Key(' ')    => Recenter,
            Key(CTRL_D)   => PageDown,
            Key(CTRL_U)   => PageUp,
            Key(CTRL_H)   => FileStart,
            Key(CTRL_L)   => FileEnd,
            // TODO: Consider changing to Mut(LineOp, cursor) for something more systematic ?
            Key('o')    => LineNew(e.view.cursor),
            //Key('O')    => LineNew(e.view.cursor), // Implement with multi command !
            Key(ENTER)  => LineBreak(e.view.cursor),
            Key('d')    => LineDel(e.view.cursor),
            Key('u')    => Undo,
            Key('U')    => Redo,
            Key('\t')   => SwitchInsert,
            Key('r')    => SwitchReplace,
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
            Key(ESC) | EscZ                 => SwitchCommand,
            Key(ENTER)                      => LineBreak(e.view.cursor),
            Key(c) if c == BACKSPACE        => Backspace(e.view.cursor),
            Key(c) if c == DEL              => Delete(e.view.cursor),
            Key(c)                          => CharInsert(e.view.cursor, c),
            _                               => Noop,
        }
    }
}



// TODO: split into movement ops, buffer ops, + misc
enum CommandOp {
    Movement(Move),
    Recenter,
    PageUp,
    PageDown,
    FileStart,
    FileEnd,
    LineDel(Pos),
    LineNew(Pos),
    LineBreak(Pos),
    Undo,
    Redo,
    Save(String),
    SwitchInsert,
    SwitchReplace,
    ClearConsole,
    Noop,
}

// CLEANUP: do I need Pos here ? Or is it implicitly wrt to the cursor of a the given view ?
enum InsertOp {
    LineBreak(Pos),
    CharInsert(Pos, char),
    Delete(Pos),
    Backspace(Pos),
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
    cursor:             Pos,
    cursor_memory:      Pos,
    filearea:           Rec,
    //selection:  Option<&[Selection]>
}

impl View {
    fn mk_fileview(filepath: String, screensize: Pos) -> View {
        View {
            filepath,
            relative_lineno:    CONF.relative_lineno,
            movement_mode:      MovementMode::Chars,
            show_token:         false,
            show_neighbor:      false,
            show_selection:     false,
            is_active:          true,
            cursor:             pos(0,0),
            cursor_memory:      pos(0,0),
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

            self.filearea = self.filearea + pos(dx, dy);
        }
    }

    fn recenter(&mut self, buffer: &Buffer) {
        let size = self.filearea.size();
        let y = max(0, self.cursor.y - size.y / 2);
        self.filearea = pos(self.cursor.x, y).extrude(size);
    }

    fn go_page_down(&mut self, buffer: &Buffer) {
        let y = min(buffer.nlines() - 1, self.cursor.y + 50);
        self.cursor = pos(self.cursor.x, y);
    }

    fn go_page_up(&mut self, buffer: &Buffer) {
        let y = max(0, self.cursor.y - 50);
        self.cursor = pos(self.cursor.x, y);
    }

    fn go_file_start(&mut self, buffer: &Buffer) {
        self.cursor = pos(self.cursor.x, 0);
    }

    fn go_file_end(&mut self, buffer: &Buffer) {
        self.cursor = pos(self.cursor.x, buffer.nlines() - 1);
    }
}




/* CORE TYPES */
mod core {


use fmt;
use std;
use std::ops::Add;
use std::ops::Neg;
use std::ops::Sub;


#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Color {
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
pub struct Colorcell {
    pub fg: Color,
    pub bg: Color,
}

pub fn colorcell(fg: Color, bg: Color) -> Colorcell {
    Colorcell { fg, bg }
}

pub fn colorcode(c : Color) -> i32 {
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


// Either a position in 2d space w.r.t to (0,0), or a movement quantity
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pos { // Vec was already taken ...
    pub x: i32,
    pub y: i32,
}

// A simple rectangle
// In general, the top-most raw and left-most column should be inclusive (min),
// and the bottom-most raw and right-most column should be exclusive (max).
#[derive(Debug, Clone, Copy)]
pub struct Rec {
    pub min: Pos,   // point the closest to (0,0)
    pub max: Pos,   // point the farthest to (0,0)
}

pub fn pos(x: i32, y: i32) -> Pos {
    Pos { x, y }
}

// TODO: rec ctor with width and height ??
pub fn rec(x0: i32, y0: i32, x1: i32, y1: i32) -> Rec {
    let (a0, a1) = ordered(x0, x1);
    let (b0, b1) = ordered(y0, y1);
    Rec {
        min: pos(a0, b0),
        max: pos(a1, b1),
    }
}

fn ordered<T>(v1: T, v2: T) -> (T, T) where T : Ord {
    if v1 < v2 {
        return (v1, v2)
    }
    (v2, v1)
}

impl Rec {
    pub fn x0(self) -> i32 { self.min.x }
    pub fn y0(self) -> i32 { self.min.y }
    pub fn x1(self) -> i32 { self.max.x }
    pub fn y1(self) -> i32 { self.max.y }
    pub fn w(self) -> i32 { self.max.x - self.min.x }
    pub fn h(self) -> i32 { self.max.y - self.min.y }

    pub fn area(self) -> i32 { self.w() * self.h() }
    pub fn size(self) -> Pos { pos(self.w(), self.h()) }

    pub fn row(self, y: i32) -> Rec {
        check!(self.min.y <= y, "row was out of bounds (left)");
        check!(y <= self.max.y, "row was out of bounds (right)");
        rec(self.min.x, y, self.max.x, y + 1)
    }

    pub fn column(self, x: i32) -> Rec {
        check!(self.min.x <= x, "column was out of bounds (top)");
        check!(x <= self.max.x, "column was out of bounds (bottom)");
        rec(x, self.min.y, x + 1, self.max.y)
    }

    // TODO: should x be forbidden from matching the bounds (i.e no empty output)
    pub fn hsplit(self, x: i32) -> (Rec, Rec) {
        check!(self.min.x <= x);
        check!(x < self.max.x);

        let left = rec(self.min.x, self.min.y, x, self.max.y);
        let right = rec(x, self.min.y, self.max.x, self.max.y);

        (left, right)
    }

    pub fn vsplit(self, y: i32) -> (Rec, Rec) {
        check!(self.min.y <= y);
        check!(y < self.max.y);

        let up = rec(self.min.x, self.min.y, self.max.x, y);
        let down = rec(self.min.x, y, self.max.x, self.max.y);

        (up, down)
    }

    // TODO: consider excluding max
    pub fn contains(self, v : Pos) -> bool {
        self.min.x <= v.x &&
        self.min.y <= v.y &&
                      v.x <= self.max.x &&
                      v.y <= self.max.y
    }
}

impl Pos {
    pub fn rec(self) -> Rec {
        Rec {
            min: pos(0,0),
            max: self,
        }
    }

    pub fn extrude(self, diag: Pos) -> Rec {
        Rec {
            min: self,
            max: self + diag,
        }
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl Add<Pos> for Pos {
    type Output = Pos;

    fn add(self, v: Pos) -> Pos {
        pos(self.x + v.x, self.y + v.y)
    }
}

impl Sub<Pos> for Pos {
    type Output = Pos;

    fn sub(self, v: Pos) -> Pos {
        pos(self.x - v.x, self.y - v.y)
    }
}

impl Neg for Pos {
    type Output = Pos;

    fn neg(self) -> Pos {
        pos(-self.x, -self.y)
    }
}

impl Add<Pos> for Rec {
    type Output = Rec;

    fn add(self, v: Pos) -> Rec {
        Rec {
            min: self.min + v,
            max: self.max + v,
        }
    }
}

impl Add<Rec> for Pos {
    type Output = Rec;

    fn add(self, r: Rec) -> Rec {
        r + self
    }
}

impl Sub<Pos> for Rec {
    type Output = Rec;

    fn sub(self, v: Pos) -> Rec {
        Rec {
            min: self.min - v,
            max: self.max - v,
        }
    }
}


} // mod core

/* UTILITIES */
#[macro_use]
mod util {


use std;
use std::cmp::max;
use std::cmp::min;
use std::error::Error;
use std::io;
use std::io::Write;
use std::fs;
use std::fmt;
use std::sync::mpsc;

use conf::CONF;


macro_rules! er {
    ($cause: expr) => {
        Err(Er { descr: format!("{}:{} cause: {}", file!(), line!(), $cause) })
    };
}


pub type Re<T> = Result<T, Er>;

#[derive(Debug)]
pub struct Er {
    pub descr: String,
}

impl fmt::Display for Er {
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

pub struct Scopeclock<'a> {
    tag: &'a str,
    timestamp: std::time::SystemTime,
}

impl <'a> Scopeclock<'a> {
    pub fn measure(tag: &'a str) -> Scopeclock {
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


pub fn itoa10(dst: &mut [u8], x: i32, padding: u8) {
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
pub fn usize(x: i32) -> usize {
    x as usize
}

pub fn i32(x: usize) -> i32 {
    x as i32
}

// CLEANUP: replace with memset if this is ever a thing in Rust
pub fn fill<T>(s: &mut [T], t: T) where T : Copy {
    for i in s.iter_mut() {
        *i = t
    }
}

pub fn copy_exact<T>(dst: &mut [T], src: &[T]) where T : Copy {
    dst.clone_from_slice(src)
}

pub fn copy<T>(dst: &mut [T], src: &[T]) where T : Copy {
    let n = min(dst.len(), src.len());
    copyn(dst, src, n)
}

pub fn copyn<T>(dst: &mut [T], src: &[T], n: usize) where T : Copy {
    dst[..n].clone_from_slice(&src[..n])
}

// CLEANUP: replace with std memchr when this make it into stable
pub fn memchr(c: u8, s: &[u8]) -> Option<usize> {
    for (i, &x) in s.iter().enumerate() {
        if x == c {
            return Some(i)
        }
    }
    None
}

pub fn clamp<'a, T>(s: &'a[T], l: usize) -> &'a[T] {
    &s[..min(l, s.len())]
}

pub fn shift<'a, T>(s: &'a[T], o: usize) -> &'a[T] {
    &s[min(o, s.len())..]
}

pub fn subslice<'a, T>(s: &'a[T], offset: usize, len: usize) -> &'a[T] {
    clamp(shift(s, offset), len)
}

// TODO: persist the file handle instead of opening/closing at every frame ...
pub fn logd<'a>(m: &'a str) {
    let mut file = fs::OpenOptions::new().create(true)
                                         .read(true)
                                         .append(true)
                                         .open(&CONF.logfile)
                                         .unwrap();
    file.write(m.as_bytes()).unwrap();
}


pub fn log(msg: &str) {
    if !CONF.debug_console {
        return
    }
    unsafe {
        CONSOLE.log(msg);
    }
}

// For the sake of simplicity, this is not wrapped into a thread_local!(RefCell::new(...)).
pub static mut CONSOLE : Debugconsole = Debugconsole {
    width:      48,
    height:     16,
    next_entry: 0,
    text:       [0; 48 * 16],
};

pub struct Debugconsole {
    pub width:      i32,
    pub height:     i32,
    pub next_entry: i32,
    pub text:       [u8; 16 * 48],
}

impl Debugconsole {
    pub fn clear() {
        unsafe {
            CONSOLE.next_entry = 0;
        }
    }

    pub fn get_line<'a>(&'a self, i: i32) -> &'a [u8] {
        let src_start = usize(self.width * (i % self.height));
        let src_stop = src_start + usize(self.width);
        &self.text[src_start..src_stop]
    }

    pub fn get_line_mut<'a>(&'a mut self, i: i32) -> &'a mut [u8] {
        let src_start = usize(self.width * (i % self.height));
        let src_stop = src_start + usize(self.width);
        &mut self.text[src_start..src_stop]
    }

    pub fn log(&mut self, msg: &str) {
        let i = self.next_entry;
        self.next_entry += 1;
        let line = self.get_line_mut(i);
        fill(line, ' ' as u8);
        copy(line, msg.as_bytes());
    }
}


} // mod util


/* DRAWING AND FRAME/SCREEN MANAGEMENT */
// TODO: nothing should directly use Framebuffer, instead always use a Screen !
mod draw {


use std::cmp::max;
use std::cmp::min;
use std::io;
use std::io::Write;
use std::mem::replace;

use util::*;
use conf::CONF;
use core::*;
use term::*;
use text::Buffer;


pub enum Draw {
    Nothing,
    All,
    Header,
    Text,
}


// The struct that manages compositing.
pub struct Framebuffer {
    window:     Pos,
    len:        i32,

    text:       Vec<u8>,
                // TODO: store u8 instead and use two tables
                // for color -> u8 -> control string conversions
    fg:         Vec<Color>,
    bg:         Vec<Color>,
    cursor:     Pos,            // Absolute screen coordinate relative to (0,0).

    buffer:     Vec<u8>,
}

const frame_default_text : u8 = ' ' as u8;
const frame_default_fg : Color = Color::Black;
const frame_default_bg : Color = Color::White;

impl Framebuffer {
    pub fn mk_framebuffer(window: Pos) -> Framebuffer {
        let len = window.x * window.y;
        let vlen = len as usize;

        Framebuffer {
            window,
            len,
            text:       vec![frame_default_text; vlen],
            fg:         vec![frame_default_fg; vlen],
            bg:         vec![frame_default_bg; vlen],
            cursor:     pos(0,0),
            buffer:     vec![0; 64 * 1024],
        }
    }

    // TODO: add clear in sub rec
    pub fn clear(&mut self) {
        fill(&mut self.text, frame_default_text);
        fill(&mut self.fg,   frame_default_fg);
        fill(&mut self.bg,   frame_default_bg);
    }

    pub fn put_line(&mut self, pos: Pos, src: &[u8]) {
        check!(self.window.rec().contains(pos));

        let maxlen = (self.window.x - pos.x) as usize;
        let len = min(src.len(), maxlen);

        let start = (pos.y * self.window.x + pos.x) as usize;
        let stop = start + len;

        copy_exact(&mut self.text[start..stop], &src[..len]);
    }

    // area.min is inclusive, area.max is exclusive
    pub fn put_color(&mut self, area: Rec, colors: Colorcell) {
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

    fn set_cursor(&mut self, new_cursor: Pos) {
        let mut x = new_cursor.x;
        let mut y = new_cursor.y;
        x = max(x, 0);
        x = min(x, self.window.x - 1);
        y = max(y, 0);
        y = min(y, self.window.y - 1);
        self.cursor = pos(x,y);
    }

    // TODO: propagate error
    // TODO: add color
    // PERF: skip unchanged sections
    pub fn render(&mut self) -> Re<()> {
        if !CONF.draw_screen {
            return Ok(())
        }

        unsafe {
            self.dump_console(&CONSOLE);
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

    fn dump_console(&mut self, console: &Debugconsole) {
        if !CONF.debug_console {
            return
        }

        let size = pos(console.width, min(console.next_entry, console.height));
        let consoleoffset = - pos(0,1); // don't overwrite the footer.
        let consolearea = Rec { min: self.window - size, max: self.window } + consoleoffset;

        let start = max(0, console.next_entry - console.height);
        for i in start..console.next_entry {
            let dst_offset = consolearea.max - pos(console.width, console.next_entry - i);
            self.put_line(dst_offset, console.get_line(i));
        }
        self.put_color(consolearea, CONF.color_console);
    }

}


// A subrectangle of a framebuffer for drawing text.
// All positions are w.r.t the Framebuffer (0,0) origin.
pub struct Screen {
    window:         Rec,
    linenoarea:     Rec,
    textarea:       Rec,
    header:         Rec,
}

impl Screen {
    pub fn mk_screen(window: Rec) -> Screen {
        let lineno_len = 5;
        let (header, filearea) = window.vsplit(1);
        let (linenoarea, textarea) = filearea.hsplit(lineno_len);

        Screen {
            window,
            linenoarea,
            textarea,
            header,
        }
    }

    pub fn draw(&self, framebuffer: &mut Framebuffer, drawinfo: &Drawinfo) {
        // TODO: use draw and only redraw what's needed
        // TODO: automatize the screen coordinate offsetting for framebuffer commands
        let file_base_offset = drawinfo.buffer_offset;
        let frame_base_offset = self.textarea.min;

        // header
        {
            framebuffer.put_line(self.header.min, drawinfo.header.as_bytes());
            framebuffer.put_color(self.header, CONF.color_header_active);
        }

        // buffer content
        {
            let y_stop = min(self.textarea.h(), drawinfo.buffer.nlines() - file_base_offset.y);
            for i in 0..y_stop {
                let lineoffset = pos(0, i);
                let file_offset = file_base_offset + lineoffset;
                let frame_offset = frame_base_offset + lineoffset;

                let mut line = drawinfo.buffer.line_get_slice(file_offset);
                line = clamp(line, self.textarea.w() as usize);
                framebuffer.put_line(frame_offset, line);
            }
        }

        // lineno
        {
            let mut buf = [0 as u8; 4];
            let lineno_base = if drawinfo.relative_lineno {
                file_base_offset.y - drawinfo.cursor.y
            } else {
                file_base_offset.y + 1
            };
            for i in 0..self.textarea.h() {
                itoa10(&mut buf, lineno_base + i, ' ' as u8);
                framebuffer.put_line(self.linenoarea.min + pos(0,i), &buf);
            }
            framebuffer.put_color(self.linenoarea, CONF.color_lineno);
        }

        // cursor
        {
            let cursor_screen_position = drawinfo.cursor + self.textarea.min - file_base_offset;
            if drawinfo.is_active {
                framebuffer.set_cursor(cursor_screen_position);
            }

            framebuffer.put_color(self.textarea.row(cursor_screen_position.y), CONF.color_cursor_lines);
            framebuffer.put_color(self.textarea.column(cursor_screen_position.x), CONF.color_cursor_lines);
        }
    }
}


// Helper data object for Screen::draw
pub struct Drawinfo<'a> {
    pub header:             &'a str,
    // TODO: replace 'buffer' and 'buffer_offset' with iterator of &[u8]
    pub buffer:             &'a Buffer,
    pub buffer_offset:      Pos,
    pub cursor:             Pos,
    pub draw:               Draw,
    pub relative_lineno:    bool,
    pub is_active:          bool,
}

} // mod draw



/* BUFFER AND TEXT MANAGEMENT */
mod text {


use std::fs;
use std::io::Read;
use std::io::Write;
use std::mem::replace;

use core::*;
use util::*;


fn range(start: usize, stop: usize) -> Range {
    check!(start <= stop);
    Range { start , stop }
}

// A pair of offsets into a buffer for delimiting lines.
#[derive(Debug, Clone, Copy)]
struct Range {
    start:  usize,      // inclusive
    stop:   usize,      // exclusive
}

impl Range {
    fn len(self) -> usize {
        self.stop - self.start
    }

    fn cut(self, n: usize) -> (Range, Range) {
        let pivot = self.start + n;
        check!(pivot <= self.stop);
        (range(self.start, pivot), range(pivot, self.stop))
    }
}


struct Line<'a> {
    range:   Range,
    text: &'a [u8],
}

impl <'a> Line<'a> {
    fn char_at(self, colno: usize) -> char {
        self.text[self.range.start + colno] as char
    }

    fn to_slice(&self) -> &'a[u8] {
        &self.text[self.range.start..self.range.stop]
    }

    fn len(&self) -> usize {
        self.range.stop - self.range.start
    }

    fn cut(&self, n: usize) -> (Range, Range) {
        self.range.cut(n)
    }
}


struct Text {
    text:   Vec<u8>,    // original content of the file when loaded
    lines:  Vec<Range>,  // subslices into the buffer or appendbuffer
}

impl Text {
    fn append(&mut self, c: char) {
        self.text.push(c as u8);
        self.lines.last_mut().unwrap().stop += 1;
    }

    fn insert(&mut self, colno: usize, c: char) {
        let (left, right) = self.lines.last().unwrap().cut(colno);
        self.append(' ');

// CLEANUP: use Vec.insert instead
        for i in (right.start..right.stop).rev() {
            self.text[i+1] = self.text[i];
        }
        self.text[right.start] = c as u8;
    }

    fn replace(&mut self, colno: usize, c: char) {
        let line = *self.lines.last().unwrap();

// CLEANUP: use Vec.insert instead ?
        if colno == line.len() {
            self.append(' ');
        }

        self.text[line.start + colno] = c as u8;
    }

    fn emptyline(&mut self) -> usize {
        self.lines.push(range(self.text.len(), self.text.len()));
        self.lines.len() - 1
    }

    fn copyline(&mut self, line_idx: usize) -> usize {
        let src = self.lines[line_idx];
        let dststart = self.text.len();
        let dststop = dststart + src.len();
        let dst = range(dststart, dststop);

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


#[derive(Clone)] // Do I need clone ??
struct Textsnapshot {
    line_indexes:   Vec<usize>, // the actual lines in the current files, as indexes into 'lines'
    dirty:          bool,
    cursor:         Pos,
}

// Manage content of a file
pub struct Buffer {
    textbuffer:             Text,
    previous_snapshots:     Vec<Textsnapshot>,
    line_indexes:           Vec<usize>,
    pub dirty:              bool,
}

// TODO: this should implement array bracket notation ?
impl Buffer {
    pub fn from_file(path: &str) -> Re<Buffer> {
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
                lines.push(range(a, b));
                a = b + 1; // skip the '\n'

//                if lines.len() == 40 {
//                    break;
//                }
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
            lines.push(range(text.len(), text.len()));
        }

        Buffer {
            textbuffer:         Text { text, lines },
            previous_snapshots: Vec::new(),
            line_indexes,
            dirty:              false,
        }
    }

    // TODO: propagate errors
    pub fn to_file(&mut self, path: &str) -> Re<()> {
        let mut f = fs::File::create(path)?;

        for i in 0..self.nlines() {
            f.write_all(self.line_get_slice(pos(0,i)))?;
            f.write_all(b"\n")?; // TODO: use platform's newline
        }

        self.dirty = false;

        Ok(())
    }

    pub fn snapshot(&mut self, cursor: Pos) {
        let line_indexes = self.line_indexes.clone();
        let dirty = self.dirty;
        self.previous_snapshots.push(Textsnapshot { line_indexes, dirty, cursor });
        self.dirty = true;
    }

    pub fn char_at(&self, lineno: usize, colno: usize) -> char {
        self.line_get(lineno).char_at(colno)
    }

    pub fn nlines(&self) -> i32 {
        i32(self.line_indexes.len())
    }

    pub fn last_line(&self) -> usize {
        self.line_indexes.len() - 1
    }

    pub fn line_len(&self, y: usize) -> usize {
        let idx = self.line_indexes[y as usize];
        self.textbuffer.lines[idx].len()
    }

    pub fn line_index(&self, lineno: usize) -> usize {
        self.line_indexes[lineno]
    }

    fn line_get(&self, lineno: usize) -> Line {
        Line {
            range: self.textbuffer.lines[self.line_index(lineno)],
            text: &self.textbuffer.text,
        }
    }

    fn line_set(&mut self, lineno: usize, range: Range) {
        let line_idx = self.line_index(lineno);
        self.textbuffer.lines[line_idx] = range;
    }

    pub fn line_get_slice<'a>(&'a self, offset: Pos) -> &'a[u8] {
        let x = offset.x as usize;
        let y = offset.y as usize;
        let line = self.line_get(y).to_slice();
        shift(line, x)
    }

    pub fn line_del(&mut self, y: usize) {
        check!(y < self.line_indexes.len());
        self.line_indexes.remove(y);

        // TODO: update all other views of that file whose cursors is below lineno
    }

    pub fn line_new(&mut self, lineno: usize) {
        let lastline = self.line_indexes.len() - 1;
        self.line_indexes.reserve(1);
// CLEANUP: use Vec.insert
        for i in (lineno..lastline).rev() {
            self.line_indexes[i+1] = self.line_indexes[i];
        }

        self.line_indexes[lineno] = self.textbuffer.emptyline();

        // TODO: update all other views of that file whose cursors is below lineno
    }

    pub fn line_break(&mut self, lineno: usize, colno: usize) {
        let (left, right) = self.line_get(lineno).cut(colno);

        self.line_new(lineno);
        let left_idx = self.line_index(lineno);
        let right_idx = self.line_index(lineno + 1);
        self.textbuffer.lines[left_idx]  = left;
        self.textbuffer.lines[right_idx] = right;

        // TODO: update all other views of that file whose cursors is below lineno
    }

    // CHECK: from/to should be inclusive
    pub fn delete(&mut self, from: Pos, to: Pos) {
        let mut y_start = usize(from.y);
        let mut y_stop = usize(to.y);

        check!(y_start <= y_stop);

        // Case 1: delete a range in a single line
        if y_start == y_stop {
            let oldlen = self.line_get(y_start).len();
            let newlen = oldlen - usize(to.x - from.x);
            let newline_idx = self.textbuffer.emptyline();
            self.textbuffer.text.reserve(newlen);

            for i in 0..usize(from.x) {
                let c =  self.line_get(y_start).char_at(i);
                self.textbuffer.append(c);
            }
            for i in usize(to.x)..oldlen {
                let c =  self.line_get(y_start).char_at(i);
                self.textbuffer.append(c);
            }

            self.line_indexes[y_start] = newline_idx;

            return
        }

        // Case 2: delete more than one line.
        //          1) trim the first line on the right
        //          2) trim the last line on the left
        //          3) delete any line in between
        if from.x != 0 {
            let (keep, _) = self.line_get(y_start).cut(usize(from.x));
            self.line_set(y_start, keep);
            y_start += 1;
        }

        if usize(to.x) != self.line_get(y_stop).len() {
            let (_, keep) = self.line_get(y_stop).cut(usize(to.x));
            self.line_set(y_stop, keep);
            y_stop -= 1;
        }

        let gap = y_stop - y_start;
        if gap > 0 {
            for i in y_stop..self.last_line() {
                let line_idx = self.line_indexes[i];
                self.line_indexes[i - gap] = line_idx;
            }
            unsafe {
                let new_nlines = usize(self.nlines()) - gap;
                self.line_indexes.set_len(new_nlines);
            }
        }
    }

    pub fn undo(&mut self) -> Option<Pos> {
        match self.previous_snapshots.pop() {
            Some(sp) => {
                self.line_indexes = sp.line_indexes;
                self.dirty = sp.dirty;
                Some(sp.cursor)
            }
            None => None,
        }
    }

    pub fn insert(&mut self, mode: InsertMode, lineno: usize, colno: usize, c: char) {
        // prepare buffer for insertion: modified line must be copied.
        // BUG: this probably needs to be only done on the first PendingInsert thing
        // actually !
        let last_idx = self.textbuffer.lines.len() - 1;
        let line_idx = self.line_index(lineno);
        if line_idx != last_idx {
            let newline_idx = self.textbuffer.copyline(line_idx);
            self.line_indexes[lineno] = newline_idx;
        }

        match mode {
            InsertMode::Insert  => self.textbuffer.insert(colno, c),
            InsertMode::Replace => self.textbuffer.replace(colno, c),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertMode {
    Insert,
    Replace,
}


fn file_load(filename: &str) -> Re<Vec<u8>> {
    let fileinfo = fs::metadata(filename)?;
    let size = fileinfo.len() as usize;

    let mut buf = vec![0; size];
    let mut f = fs::File::open(filename)?;

    let nread = f.read(&mut buf)?;
    if nread != size {
        return er!("not enough bytes");
    }

    Ok(buf)
}


} // mod text


/* COMMAND AND BUFFER MANIPULATION */
impl Commandstate {
    fn do_command(&mut self, op: CommandOp, e: &mut Editor) -> Re<Mode> {
        use CommandOp::*;
        use Mode::*;
        match op {
            Movement(mvt)=>
                e.mv_cursor(mvt),

            Recenter =>
                e.view.recenter(&e.buffer),

            PageUp =>
                e.view.go_page_up(&e.buffer),

            PageDown =>
                e.view.go_page_down(&e.buffer),

            FileStart =>
                e.view.go_file_start(&e.buffer),

            FileEnd =>
                e.view.go_file_end(&e.buffer),

            LineDel(pos) => {
                if e.buffer.nlines() > 0 {
                    let lineno = usize(pos.y);
                    e.buffer.snapshot(e.view.cursor);
                    e.buffer.line_del(lineno);
                }
            }

            LineNew(pos) => {
                let lineno = usize(pos.y);
                e.buffer.snapshot(e.view.cursor);
                e.buffer.line_new(lineno);
            }

            LineBreak(Pos { x, y }) => {
                let lineno = usize(y);
                let colno = usize(x);
                e.buffer.snapshot(e.view.cursor);
                e.buffer.line_break(lineno, colno);
                e.view.cursor = pos(0, y + 1);
            }

            Undo => {
                match e.buffer.undo() {
                    Some(p) => e.view.cursor = p,
                    None => (),
                };
            }

            Redo => (), // TODO: implement redo stack

            Save(path) => e.buffer.to_file(&path)?,

            ClearConsole => Debugconsole::clear(),

            SwitchInsert => {
                let mode = InsertMode::Insert;
                return Ok(PendingInsert(mode))
            }

            SwitchReplace => {
                let mode = InsertMode::Replace;
                return Ok(PendingInsert(mode))
            }

            Noop => (),
        }

        Ok(Command(replace(self, Commandstate { })))
    }
}

impl Insertstate {
    fn do_insert(&mut self, op: InsertOp, e: &mut Editor) -> Re<Mode> {
        use InsertOp::*;
        match op {
            LineBreak(Pos { x, y }) => {
                e.buffer.line_break(usize(y), usize(x));
                e.view.cursor = pos(0, y + 1);
            }

            CharInsert(Pos { x, y }, c) => {
                if is_printable(c) {
                    e.buffer.insert(self.mode, usize(y), usize(x), c);
                    e.view.cursor = pos(x + 1, y);
                    // TODO: think about auto linebreak
                }
            }

            Delete(p) => {
                // BUG: this should use char next instead, for handling end of line
                e.buffer.delete(p, p + pos(1,0))
            }

            Backspace(p) => {
                // BUG: this should use char previous instead, for handling start of line
                if p.x > 0 {
                    let newpos = p - pos(1,0);
                    e.buffer.delete(newpos, p);
                    e.view.cursor = newpos;
                }
            }

            SwitchCommand =>
                return Ok(Mode::default_command_state),

            Noop => (),
        }

        // TODO: can I  just return something new and dorp self here ???
        Ok(Mode::Insert(replace(self, Insertstate { mode: InsertMode::Insert })))
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

        std::thread::spawn(move || {
            push_char(&send);
        });


        while m != Mode::Exit {
            let i = pull_input(&recv)?;
            log(&format!("input: {}", i));

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

            let header = format!("{}{} {:?}",
                    self.view.filepath,
                    if self.buffer.dirty { "+" } else { " " },
                    self.view.movement_mode);
            let drawinfo = Drawinfo {
                header:             &header,
                buffer:             &self.buffer,
                buffer_offset:      self.view.filearea.min,
                cursor:             self.view.cursor,
                draw:               Draw::All,
                relative_lineno:    self.view.relative_lineno,
                is_active:          self.view.is_active,
            };
            let screen = Screen::mk_screen(self.mainscreen); //TODO: persist in Editor
            screen.draw(&mut self.framebuffer, &drawinfo);
        }

        // footer
        {
            self.framebuffer.put_line(self.footer.min + pos(1,0), mode.name().as_bytes());
            self.framebuffer.put_line(self.footer.min + pos(10, 0), b"FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER");
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
            Left  => pos(-1,0),
            Right => pos(1,0),
            Up    => pos(0,-1),
            Down  => pos(0,1),
            _     => pos(0,0),
        };

        // TODO: update the 'desired cursor position' instead of the real cursor position
        self.view.cursor = self.view.cursor + delta;
    }

    fn resize(&mut self) {
        // TODO
    }
}


/* TERMINAL BINDINGS */
mod term {


use std::fmt;
use std::cmp::max;
use std::cmp::min;
use std::error::Error;
use std::io;
use std::io::Read;
use std::io::Write;
use std::panic;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;

use conf::CONF;
use core::*;
use util::*;


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
pub struct Term {
}

impl Drop for Term {
    fn drop(&mut self) {
        Term::restore();
    }
}

impl Term {
    pub fn size() -> Pos {
        unsafe {
            let ws = terminal_get_size();
            pos(ws.ws_col as i32, ws.ws_row as i32)
        }
    }

    pub fn set_raw() -> Re<Term> {
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
            let std_panic_hook = panic::take_hook();
            panic::set_hook(Box::new(move |panicinfo| {
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


// CLEANUP: this should not have to be exposed
pub const term_start                      : &[u8] = b"\x1b[";
pub const term_finish                     : &[u8] = b"\x1b[0m";
pub const term_clear                      : &[u8] = b"\x1bc";
pub const term_cursor_hide                : &[u8] = b"\x1b[?25l";
pub const term_cursor_show                : &[u8] = b"\x1b[?25h";
pub const term_cursor_save                : &[u8] = b"\x1b[s";
pub const term_cursor_restore             : &[u8] = b"\x1b[u";
pub const term_switch_offscreen           : &[u8] = b"\x1b[?47h";
pub const term_switch_mainscreen          : &[u8] = b"\x1b[?47l";
pub const term_switch_mouse_event_on      : &[u8] = b"\x1b[?1000h";
pub const term_switch_mouse_tracking_on   : &[u8] = b"\x1b[?1002h";
pub const term_switch_mouse_tracking_off  : &[u8] = b"\x1b[?1002l";
pub const term_switch_mouse_event_off     : &[u8] = b"\x1b[?1000l";
pub const term_switch_focus_event_on      : &[u8] = b"\x1b[?1004h";
pub const term_switch_focus_event_off     : &[u8] = b"\x1b[?1004l";
pub const term_gohome                     : &[u8] = b"\x1b[H";
pub const term_newline                    : &[u8] = b"\r\n";


/* KEY INPUT HANDLING */

// TODO: pretty print control codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Noinput,
    Key(char),
    Click(Pos),
    ClickRelease(Pos),
    UnknownEscSeq,
    EscZ,       // shift + tab -> "\x1b[Z"
    Resize,
    Error,
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Input::*;
        match self {
            Noinput                         => f.write_str(&"Noinput"),
            UnknownEscSeq                   => f.write_str(&"Unknown"),
            EscZ                            => f.write_str(&"EscZ"),
            Resize                          => f.write_str(&"Resize"),
            Error                           => f.write_str(&"Error"),
            Key(c)                          => Input::fmt_key_name(*c, f),
            Click(Pos { x, y })             => write!(f, "click ({},{})'", y, x),
            ClickRelease(Pos { x, y })      => write!(f, "unclick ({},{})'", y, x),
        }
    }
}

impl Input {
    fn key_descr(c: char) -> Option<&'static str> {
        let r = match c {
            CTRL_AT             => &"^@",
            CTRL_A              => &"^A",
            CTRL_B              => &"^B",
            CTRL_C              => &"^C",
            CTRL_D              => &"^D",
            CTRL_E              => &"^E",
            CTRL_F              => &"^F",
            CTRL_G              => &"^G",
            BACKSPACE           => &"Backspace",
            TAB                 => &"TAB",
            CTRL_J              => &"^J",
            CTRL_K              => &"^K",
            CTRL_L              => &"^L",
            ENTER               => &"Enter",
            CTRL_N              => &"^N",
            CTRL_O              => &"^O",
            CTRL_P              => &"^P",
            CTRL_Q              => &"^Q",
            CTRL_R              => &"^R",
            CTRL_S              => &"^S",
            CTRL_T              => &"^T",
            CTRL_U              => &"^U",
            CTRL_V              => &"^V",
            CTRL_W              => &"^W",
            CTRL_X              => &"^X",
            CTRL_Y              => &"^Y",
            CTRL_Z              => &"^Z",
            ESC                 => &"Esc",
            CTRL_BACKSLASH      => &"^\\",
            CTRL_RIGHT_BRACKET  => &"^]",
            CTRL_CARET          => &"^^",
            CTRL_UNDERSCORE     => &"^_",
            DEL                 => &"Del",
            _                   => return None,
        };
        Some(r)
    }

    fn fmt_key_name(c: char, f: &mut fmt::Formatter) -> fmt::Result {
        match Input::key_descr(c) {
            Some(s) => f.write_str(s),
            None    => write!(f,"{}", c),
        }
    }
}


pub const CTRL_AT               : char = '\x00';
pub const CTRL_A                : char = '\x01';
pub const CTRL_B                : char = '\x02';
pub const CTRL_C                : char = '\x03';
pub const CTRL_D                : char = '\x04';
pub const CTRL_E                : char = '\x05';
pub const CTRL_F                : char = '\x06';
pub const CTRL_G                : char = '\x07';
pub const CTRL_H                : char = '\x08';
pub const CTRL_I                : char = '\x09';
pub const CTRL_J                : char = '\x0a';
pub const CTRL_K                : char = '\x0b';
pub const CTRL_L                : char = '\x0c';
pub const CTRL_M                : char = '\x0d';
pub const CTRL_N                : char = '\x0e';
pub const CTRL_O                : char = '\x0f';
pub const CTRL_P                : char = '\x10';
pub const CTRL_Q                : char = '\x11';
pub const CTRL_R                : char = '\x12';
pub const CTRL_S                : char = '\x13';
pub const CTRL_T                : char = '\x14';
pub const CTRL_U                : char = '\x15';
pub const CTRL_V                : char = '\x16';
pub const CTRL_W                : char = '\x17';
pub const CTRL_X                : char = '\x18';
pub const CTRL_Y                : char = '\x19';
pub const CTRL_Z                : char = '\x1a';
pub const CTRL_LEFT_BRACKET     : char = '\x1b';
pub const CTRL_BACKSLASH        : char = '\x1c';
pub const CTRL_RIGHT_BRACKET    : char = '\x1d';
pub const CTRL_CARET            : char = '\x1e';
pub const CTRL_UNDERSCORE       : char = '\x1f';
pub const DEL                   : char = '\x7f';
pub const ESC                   : char = CTRL_LEFT_BRACKET;
pub const BACKSPACE             : char = CTRL_H;
pub const TAB                   : char = CTRL_I;
pub const LINE_FEED             : char = CTRL_J;
pub const VTAB                  : char = CTRL_K;
pub const NEW_PAGE              : char = CTRL_L;
pub const ENTER                 : char = CTRL_M;

pub const RESIZE                : char = 255 as char; //'\xff';

pub fn is_printable(c : char) -> bool {
    ESC < c && c < DEL
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


pub fn push_char(chan: &SyncSender<char>) {
    let mut stdin = io::stdin();
    let mut buf = [0;1];
    // TODO: handle interrupts when errno == EINTR
    // TODO: support unicode !
    loop {
        let n = stdin.read(&mut buf).unwrap(); // TODO: pass error through the channel ?
        if n == 1 {
            let c = buf[0];
            let d = match Input::key_descr(c as char) {
                Some(s) => logd(&format!("input: {}/{}\n", c, s)),
                None    => logd(&format!("input: {}/{}\n", c, c as char)),
            };
            chan.send(buf[0] as char).unwrap();
        }
    }
}

pub fn pull_input(chan: &Receiver<char>) -> Re<Input> {
    use Input::*;
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
        Ok(c) if c == '['       => (),                          // Escape sequence: continue parsing
        Ok(c)                   => return Ok(UnknownEscSeq),    // Error while parsing: bail out
        Err(Empty)              => return Ok(Key(ESC)),         // Nothing more: this was just an escape key
        Err(e)                  => return er!(e.description()),
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

    let p = pos(x,y);

    let r = match c2 & 3 /* ignore modifier keys */ {
        0 ... 2 =>  Click(p),
        3       =>  ClickRelease(p),
        _       =>  UnknownEscSeq,
    };

    Ok(r)
}

} // mod term
