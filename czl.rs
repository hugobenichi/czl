#![allow(dead_code)]
#![allow(non_upper_case_globals)]


use std::cmp::max;
use std::cmp::min;

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
 * Buffer operation migration
 *  - CommandOps/BufferOps: always take a snapshot and only push the snapshot if Opresult is not
 *  noop
 *  - regroup CommandOps on buffer and InsertOps ?
 *  - CommandOps: composite ops
 *      join line range,
 *      delete line range,
 *      delete range
 *      delete vertical range:w
 *      cut line section
 *          => delete and backspace in command mode
 *      replace line section,
 *  - debug redo() and make sure undo/redo works
 *  - undo bug: backspace on undo on line boundary in insert mode
 *  - undo bug: in normal mode backspace on first char, then backspace again, undo does not bring
 *  back line immediately (but eventually restore text after one more undo)
 *  - undo bug: when deleting last char on line in command mode, undo does not bring back that char
 *  - better track dity flag:
 *      once everything use Opresult: fix dirty bugs and history bugs when a noop happens
 *
 * Features:
 *  - offer to save if panic
 *  - better navigation !
 *  - copy and yank buffer
 *  - cursor horizontal memory
 *  - buffer explorer
 *  - directory explorer
 *  - grep move
 *  - cursor previous points and cursor markers
 *  - ctags support
 *  - support tab character and autodetect tab expansion ?
 *  - add a special input for forcing a tab insert
 *
 * TODOs and cleanups
 *  - PERF add clear in sub rec to framebuffer and use Draw in Drawinfo to redraw only what's needed
 *  - fuzzer
 *  - handle resize
 *  - utf8 support: Range and Filebuffer, Input, ... don't wait too much
 */


fn main() {
    let _term = Term::set_raw().unwrap();

    open_logfile(&CONF.logfile).unwrap();

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

    color_default:          Colorcell { fg: Color::Black,   bg: Color::White },
    color_header_active:    Colorcell { fg: Color::Black,   bg: Color::Yellow },
    color_header_inactive:  Colorcell { fg: Color::Gray(2), bg: Color::Cyan },
    color_footer:           Colorcell { fg: Color::White,   bg: Color::Gray(14) },
    color_lineno:           Colorcell { fg: Color::Green,   bg: Color::White },
    color_console:          Colorcell { fg: Color::White,   bg: Color::Gray(12) },
    color_cursor_lines:     Colorcell { fg: Color::Black,   bg: Color::Gray(15) },

    color_mode_command:     Colorcell { fg: Color::BoldWhite, bg: Color::Black },
    color_mode_insert:      Colorcell { fg: Color::BoldWhite, bg: Color::Red },
    color_mode_replace:     Colorcell { fg: Color::BoldWhite, bg: Color::Magenta },
    color_mode_exit:        Colorcell { fg: Color::Magenta, bg: Color::Magenta },

    tab_expansion:          4,

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
    pub color_mode_replace:     Colorcell,
    pub color_mode_exit:        Colorcell,

    pub tab_expansion:          i32,

    pub logfile:                &'static str,
}


} // mod conf




/* CORE TYPES */
mod core {


use std::fmt;
use std::ops::Add;
use std::ops::AddAssign;
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

pub fn colorcode(c : Color) -> i32 {
    use Color::*;
    match c {
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
        RGB216 { r, g, b }       => 16 + (b + 6 * (g + 6 * r)),
        Gray(g)                  => 232 + g,
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

    pub fn usize(self) -> (usize, usize) {
        (self.x as usize, self.y as usize)
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

impl AddAssign<Pos> for Pos {
    fn add_assign(&mut self, v: Pos) {
        self.x += v.x;
        self.y += v.y;
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
        logconsole(&format!("{}: {}.{:06}", self.tag, dt.as_secs(), dt.subsec_nanos() / 1000));
    }
}


pub fn itoa10_right(dst: &mut [u8], x: i32, padding: u8) {
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

pub fn itoa10_left(dst: &mut [u8], x: i32) -> usize {
    // Does not handle negative numbers
    let mut n = 0;
    let mut y = x.abs();
    while y != 0 {
        y /= 10;
        n += 1;
    }
    y = x;
    let n_digits = n;
    for i in (0..n).rev() {
        dst[i] = (y % 10) as u8 + ('0' as u8);
        y /= 10;
    }

    n_digits
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

pub fn clamp<'a, T>(s: &'a[T], l: usize) -> &'a[T] {
    &s[..min(l, s.len())]
}

pub fn shift<'a, T>(s: &'a[T], o: usize) -> &'a[T] {
    &s[min(o, s.len())..]
}

pub fn subslice<'a, T>(s: &'a[T], offset: usize, len: usize) -> &'a[T] {
    clamp(shift(s, offset), len)
}


static mut logfile : Option<fs::File> = None;

pub fn open_logfile(filename: &str) -> Re<()> {
    let file = fs::OpenOptions::new().create(true)
                                     .read(true)
                                     .append(true)
                                     .open(filename)?;
    unsafe {
        logfile = Some(file);
    }

    Ok(())
}

pub fn logd(m: &str) {
    unsafe {
        match logfile {
            Some(ref mut f) => {
                let _ = f.write(m.as_bytes());
            }
            None => (),
        }
    }
}


pub fn logconsole(msg: &str) {
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




mod ioutil {

use std::fs;
use util::*;
use std::io::Read;

pub fn file_load(filename: &str) -> Re<Vec<u8>> {
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


} // mod ioutil




/* TERMINAL BINDINGS */
mod term {


use std::fmt;
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
    ws_row:     u16,
    ws_col:     u16,
    ws_xpixel:  u16,
    ws_ypixel:  u16,
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
            h.write(b"\x1b[s")?;            // save cursor
            h.write(b"\x1b[?47h")?;         // go offscreen
            h.write(b"\x1b[?1000h")?;       // get mouse event
            h.write(b"\x1b[?1002h")?;       // track mouse event
            h.write(b"\x1b[?1004h")?;       // get focus event
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
            if CONF.no_raw_mode || !is_raw {
                return
            }
        }

        let stdout = io::stdout();
        let mut h = stdout.lock();
        h.write(b"\x1b[?1004l").unwrap();   // stop focus event
        h.write(b"\x1b[?1002l").unwrap();   // stop mouse tracking
        h.write(b"\x1b[?1000l").unwrap();   // stop mouse event
        h.write(b"\x1b[?47l").unwrap();     // go back to main screen
        h.write(b"\x1b[u").unwrap();        // restore cursor
        h.flush().unwrap();

        unsafe {
            terminal_restore();
            is_raw = false;
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Noinput,
    Error,
    UnknownEscSeq,
    Key(char),
    Click(Pos),
    ClickRelease(Pos),
    EscZ,               // shift + tab -> "\x1b[Z"
    Resize,
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Input::*;
        match self {
            Noinput                         => f.write_str(&"Noinput"),
            Error                           => f.write_str(&"Error"),
            UnknownEscSeq                   => f.write_str(&"Unknown"),
            Key(c)                          => Input::fmt_key_name(*c, f),
            Click(Pos { x, y })             => write!(f, "click ({},{})'", y, x),
            ClickRelease(Pos { x, y })      => write!(f, "unclick ({},{})'", y, x),
            EscZ                            => f.write_str(&"EscZ"),
            Resize                          => f.write_str(&"Resize"),
        }
    }
}

impl Input {
    // return Some(name) for keys with a special description
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
            SPACE               => &"Space",
            DEL                 => &"Del",
            _                   => return None,
        };
        Some(r)
    }

    fn fmt_key_name(c: char, f: &mut fmt::Formatter) -> fmt::Result {
        match Input::key_descr(c) {
            Some(s) => f.write_str(s),
            None    => write!(f, "{}", c),
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
pub const SPACE                 : char = '\x20';
pub const DEL                   : char = '\x7f';
pub const ESC                   : char = CTRL_LEFT_BRACKET;
pub const BACKSPACE             : char = CTRL_H;
pub const TAB                   : char = CTRL_I;
pub const LINE_FEED             : char = CTRL_J;
pub const VTAB                  : char = CTRL_K;
pub const NEW_PAGE              : char = CTRL_L;
pub const ENTER                 : char = CTRL_M;

// Special code
pub const RESIZE                : char = 255 as char; //'\xff';


pub fn is_printable(c : char) -> bool {
    SPACE <= c && c < DEL
}

pub fn push_char(chan: &SyncSender<char>) {
    let mut stdin = io::stdin();
    let mut buf = [0;1];
    // TODO: handle interrupts when errno == EINTR
    // TODO: support unicode !
    loop {
        let n = stdin.read(&mut buf).unwrap(); // TODO: pass error through the channel ?
        if n == 1 {
            let c = buf[0];
            match Input::key_descr(c as char) {
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
        Ok(_)                   => return Ok(UnknownEscSeq),    // Error while parsing: bail out
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




/* DRAWING AND FRAME/SCREEN MANAGEMENT */
mod draw {


use std::cmp::max;
use std::cmp::min;
use std::io;
use std::io::Write;
use std::mem::replace;

use conf::CONF;
use text::Buffer;
use util::*;
use core::*;


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    Nothing,            // nothing to redraw
    Header,             // only redraw header
    All,                // redraw all text and header
}

// The struct that manages compositing.
pub struct Framebuffer {
    window:     Pos,
    text:       Vec<u8>,
    fg:         Vec<i32>,
    bg:         Vec<i32>,
    cursor:     Pos,            // Absolute screen coordinate relative to (0,0).
    buffer:     Vec<u8>,        // used for storing frame data before writing to the terminal
}

const frame_default_text : u8 = ' ' as u8;
const frame_default_fg : i32 = 0; // Black
const frame_default_bg : i32 = 7; // White

impl Framebuffer {
    pub fn mk_framebuffer(window: Pos) -> Framebuffer {
        let len = usize(window.x * window.y);

        Framebuffer {
            window,
            text:       vec![frame_default_text; len],
            fg:         vec![frame_default_fg; len],
            bg:         vec![frame_default_bg; len],
            cursor:     pos(0,0),
            buffer:     vec![0; 64 * 1024],
        }
    }

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

        for _ in y0..y1 {
            fill(&mut self.fg[x0..x1], colorcode(colors.fg));
            fill(&mut self.bg[x0..x1], colorcode(colors.bg));
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

        append(&mut buffer, b"\x1b[?25l");  // hide cursor
        append(&mut buffer, b"\x1b[H");     // go home

        let w = self.window.x as usize;
        let mut l = 0;
        let mut r = w;
        let mut numbuf1 = [0 as u8; 8];
        let mut numbuf2 = [0 as u8; 8];

        for i in 0..self.window.y {
            if i > 0 {
                // Do not put "\r\n" on the last line
                append(&mut buffer, b"\r\n");
            }


            if CONF.draw_colors {
                let mut j = l;
                loop {
                    let k = self.find_color_end(j, r);

                    { // fg color
                        append(&mut buffer, b"\x1b[38;5;");
                        let n = itoa10_left(&mut numbuf1, self.fg[j]);
                        append(&mut buffer, &numbuf1[..n]);
                    }

                    { // bg color
                        append(&mut buffer, b";48;5;");
                        let n = itoa10_left(&mut numbuf2, self.bg[j]);
                        append(&mut buffer, &numbuf2[..n]);
                    }

                    append(&mut buffer, b"m");
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

        // cursor
        {
            // Terminal cursor coodinates start at (1,1)
            let y_n = itoa10_left(&mut numbuf1, self.cursor.y + 1);
            let x_n = itoa10_left(&mut numbuf2, self.cursor.x + 1);
            append(&mut buffer, b"\x1b[");
            append(&mut buffer, &numbuf1[..y_n]);
            append(&mut buffer, b";");
            append(&mut buffer, &numbuf2[..x_n]);
            append(&mut buffer, b"H");
        }

        //let cursor_command = format!("\x1b[{};{}H", self.cursor.y + 1, self.cursor.x + 1);
        //append(&mut buffer, cursor_command.as_bytes());
        append(&mut buffer, b"\x1b[?25h");

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

    pub fn put_text(&self, framebuffer: &mut Framebuffer, drawinfo: &Drawinfo) {
        let file_base_offset = drawinfo.buffer_offset;
        let frame_base_offset = self.textarea.min;

        if drawinfo.draw == Draw::Nothing {
            return
        }

        // header
        {
            framebuffer.put_line(self.header.min, drawinfo.header.as_bytes());
            framebuffer.put_color(self.header, CONF.color_header_active);
        }

        // FUTURE: this will need to be skipped for an inactive view sharing a buffer if the active
        // view has pushed a change show that this inactive view should update its cursor.
        if drawinfo.draw == Draw::Header {
            return
        }

        // buffer content
        {
            let y_stop = min(self.textarea.h(), drawinfo.buffer.nlines() - file_base_offset.y);
            for (i, line) in drawinfo.buffer.iter(drawinfo.buffer_offset, y_stop).enumerate() {
                let frame_offset = frame_base_offset + pos(0, i32(i));
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
                itoa10_right(&mut buf, lineno_base + i, ' ' as u8);
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


use std;
use std::cmp::min;
use std::fs;
use std::io::Write;
use std::mem::swap;

use core::*;
use util::*;
use ioutil;


#[cfg(windows)]
const LINE_ENDING: &'static [u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &'static [u8] = b"\n";


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


// Manage content of a file
pub struct Buffer {
    text:                   Vec<u8>,
    lines:                  Vec<Range>,
    pub dirty:              bool,

    opbuffer:                    OpBuffer,
    snapshots:              Vec<Snapshot>,
    snapshot_next:          usize,

    // TODO: should this track the current insert / command mode ?
}

pub struct BufferIter<'a> {
    buffer: &'a Buffer,
    offset: Pos,
    nlines: i32,
}

impl <'a> Iterator for BufferIter<'a> {
    type Item = &'a[u8];

    fn next(&mut self) -> Option<&'a[u8]> {
        if self.nlines < 1 {
            return None
        }

        let offset = self.offset;
        self.nlines -= 1;
        self.offset += pos(0,1);

        Some(self.buffer.line_get_slice(offset))
    }
}

impl Buffer {

    pub fn iter(&self, offset: Pos, want_nlines: i32) -> BufferIter {
        let nlines = min(want_nlines, self.nlines() - offset.y);
        BufferIter {
            buffer: self,
            offset,
            nlines,
        }
    }

    pub fn iter_all(&self) -> BufferIter {
        self.iter(pos(0,0), self.nlines())
    }

    pub fn from_file(path: &str) -> Re<Buffer> {
        let text = ioutil::file_load(path)?;
        Ok(Buffer::from_text(text))
    }

    pub fn from_text(text: Vec<u8>) -> Buffer {

        let mut lines = Vec::new();

        let mut a = 0;
        let newline = '\n' as u8;
        for (_, line) in text.split(|c| *c == newline).enumerate() {
            let l = line.len();
            let b = a + l;
            let mut r = range(a, b);
            a = b + 1;
            if l > 1 && line[l - 2] == '\r' as u8 {
                r.stop -= 1;
            }
            lines.push(r);
        }

        Buffer {
            text,
            lines,
            dirty:              false,
            snapshots:          Vec::new(),
            snapshot_next:      0,
            opbuffer:           OpBuffer {
                ops:                Vec::new(),
                cursor:             0,
                pending:            0,
            }
        }
    }

    pub fn to_file(&mut self, path: &str) -> Re<()> {
        let mut f = fs::File::create(path)?;
        let mut line_ending : &[u8] = &vec!();
        for line in self.iter_all() {
            f.write_all(line_ending)?;
            f.write_all(line)?;
            line_ending = LINE_ENDING;
        }

        self.dirty = false;

        Ok(())
    }

    pub fn snapshot(&mut self, cursor: Pos) {
        let s = Snapshot {
            text_cursor:    self.text.len(),
            op_cursor:      self.opbuffer.cursor,
            dirty:          self.dirty,
            cursor:         cursor,
        };
        self.snapshots.push(s);
        self.snapshot_next += 1;
        self.dirty = true;
    }

    pub fn char_at(&self, lineno: usize, colno: usize) -> char {
        // UTF8: need to iterate from line start
        self.line_get(lineno).char_at(colno)
    }

    pub fn nlines(&self) -> i32 {
        i32(self.lines.len())
    }

    pub fn line_last(&self) -> usize {
        self.lines.len() - 1
    }

    pub fn line_len(&self, lineno: usize) -> usize {
        // UTF8: count number of chars
        self.lines[lineno].len()
    }

    fn line_get(&self, lineno: usize) -> Line {
        Line {
            range: self.lines[lineno],
            text: &self.text,
        }
    }

    fn line_set(&mut self, lineno: usize, range: Range) {
        self.lines[lineno] = range;
    }

    fn line_get_slice<'a>(&'a self, offset: Pos) -> &'a[u8] {
        let x = usize(offset.x);
        let y = usize(offset.y);
        let line = self.line_get(y).to_slice();
        shift(line, x)
    }

    pub fn line_del(&mut self, p: Pos) -> Opresult {
        if self.nlines() == 0 {
            return Opresult::Noop
        }

        let lineno = usize(p.y);
        check!(lineno < self.lines.len());

        self.push_op(Op {
            lineno,
            line:           Range { start: 0, stop: 0 },
            op_type:        Optype::Del,
        });

        Opresult::Change(p)
    }

    fn line_empty(&mut self) -> Range {
        range(self.text.len(), self.text.len())
    }

    pub fn line_new(&mut self, p: Pos) -> Opresult {
        let lineno = usize(p.y);
        let line = self.line_empty();

        self.push_op(Op { lineno, line, op_type:Optype::Ins });

        Opresult::Change(p)
    }

    pub fn line_join(&mut self, p: Pos) -> Opresult {
        let lineno = usize(p.y);
        let start = self.text.len();

        let line1 = self.lines[lineno];
        let line2 = self.lines[lineno + 1];
        self.text_copy(line1);
        self.text_copy(line2);

        let line = range(start, start + line1.len() + line2.len());

        self.push_op(Op {
            lineno:     lineno,
            line:       line,
            op_type:    Optype::Rep,
        });
        self.line_del(p + pos(0,1));

        Opresult::Change(p)
    }

    pub fn line_break(&mut self, p: Pos) -> Opresult {
        let (colno, lineno) = p.usize();
        let (left, right) = self.line_get(lineno).cut(colno);

        self.push_op(Op {
            lineno:     lineno,
            line:       left,
            op_type:    Optype::Rep,
        });
        self.push_op(Op {
            lineno:     lineno + 1,
            line:       right,
            op_type:    Optype::Ins,
        });

        Opresult::Change(pos(0, p.y + 1))
    }

    fn text_copy(&mut self, r: Range) {
        self.text.reserve(r.len());
        for i in r.start..r.stop {
            let c = self.text[i];
            self.text.push(c);
        }
    }

    fn cloneline(&mut self, lineno: usize) -> Range {
        let start = self.text.len();
        let src = self.lines[lineno];
        self.text_copy(src);

        range(start, start + src.len())
    }

    pub fn prepare_insert(&mut self, lineno: usize) {
        let line = self.cloneline(lineno);
        self.push_op(Op { lineno, line, op_type: Optype::Rep });
    }

    pub fn char_insert(&mut self, mode: InsertMode, p: Pos, c: char) -> Opresult {
        let (colno, lineno) = p.usize();
        // check that we are operating in Insert mode !
        // TODO: think about auto linebreak
        match mode {
            InsertMode::Insert  => {
                let line = &mut self.lines[lineno];
                line.stop += 1;
                self.text.insert(line.start + colno, c as u8);
            }
            InsertMode::Replace => {
                let line = &mut self.lines[lineno];
                if colno == line.len() {
                    self.text.push(c as u8);
                    line.stop += 1;
                } else {
                    self.text[line.start + colno] = c as u8;
                }
            }
        }

        Opresult::Change(p + pos(1, 0))
    }

    pub fn char_delete(&mut self, cursor: Pos) {
        let (colno, lineno) = cursor.usize();

        if self.lines[lineno].stop != self.text.len() {
            self.prepare_insert(lineno);
        }

        let Range { start, stop } = self.lines[lineno];
        let linelen = stop - start - 1;
        for i in colno..linelen {
            let c =  self.text[start + i + 1];
            self.text[start + i] = c;
        }
        self.lines[lineno].stop -= 1;
    }

    pub fn del(&mut self, cursor: Pos) -> Opresult {
        // CLEANUP: try to return the result of line_del, line_join, ...
        let (colno, lineno) = cursor.usize();
        let len = self.line_len(lineno);

        // current line is empty
        if len == 0 {
            self.line_del(cursor);
            return Opresult::Change(cursor)
        }

        // last char in file
        if lineno == self.line_last() && colno == len - 1 {
            // BUG this does not work correctly in command mode and eats a character
            self.char_delete(cursor);
            return Opresult::Change(cursor - pos(1,0))
        }

        // last char on line
        if colno == len - 1 {
            // next line is empty
            if self.line_len(lineno + 1) == 0 {
                self.line_del(cursor + pos(0,1));
                return Opresult::Change(cursor)
            }

            // else join lines
            // BUG: use a line op here
            self.lines[lineno].stop -= 1;
            self.line_join(cursor);
            return Opresult::Change(cursor)
        }

        self.char_delete(cursor);

        Opresult::Change(cursor)
    }

    pub fn backspace(&mut self, cursor: Pos) -> Opresult {
        // CLEANUP: try to return the result of line_del, line_join, ...
        // first line, first char: noop
        if cursor == pos(0,0) {
            return Opresult::Noop
        }

        let mut cursor_prev = cursor - pos(1,0);
        if cursor_prev.x < 0 {
            let prev_len = self.line_len(usize(cursor.y) - 1);
            cursor_prev = pos(i32(prev_len), cursor.y - 1);
        }
        let r = Opresult::Change(cursor_prev);

        let lineno = usize(cursor.y);
        let len = self.line_len(lineno);

        // current line is empty
        if len == 0 {
            self.line_del(cursor);
            return r
        }

        // beggining of line and previous line is empty
        if cursor.x == 0 && self.line_len(lineno - 1) == 0 {
            self.line_del(cursor - pos(0,1));
            return r
        }

        // beggining of line: join
        if cursor.x == 0 {
            self.line_join(cursor - pos(0,1));
            return r
        }

        self.char_delete(cursor_prev);

        r
    }

    pub fn undo(&mut self) -> Opresult {
        if self.snapshots.is_empty() {
            return Opresult::Noop
        }

        let s = self.snapshots.pop().unwrap();
        self.ops_undo(s.op_cursor);
        self.dirty = s.dirty;
        // TODO: introduce text cursor and adjust it here
        Opresult::Change(s.cursor)
    }

    pub fn redo(&mut self) -> Opresult {
        if self.snapshot_next == self.snapshots.len() {
            return Opresult::Noop
        }

        let s = self.snapshots[self.snapshot_next];

        self.ops_redo(s.op_cursor);

        // CLEANUP: this should be done as an Opresult ?
        // TODO adjust text cursor
        self.dirty = s.dirty;
        self.snapshot_next += 1;
        Opresult::Change(s.cursor)
    }

    fn push_op(&mut self, op: Op) {
        self.opbuffer.ops.insert(self.opbuffer.pending, op);
        self.opbuffer.pending += 1;
    }

    fn pending_ops(&mut self) -> std::ops::Range<usize> {
        (self.opbuffer.cursor..self.opbuffer.pending)
    }

    pub fn ops_do(&mut self) {
        // 1) need to move the thing out first ...
        // 2) or I can make an index range instead and only copy grab the op.op_type by value ...
        for i in self.pending_ops() {
            let op_type;
            let lineno;
            let line;
            {
                let op = self.opbuffer.ops[i];
                op_type = op.op_type;
                lineno = op.lineno;
                line = op.line;
            }
            match op_type {
                Optype::Del => {
                    self.opbuffer.ops[i].line = line;
                }
                Optype::Ins => {
                    self.lines.insert(lineno,line);
                }
                Optype::Rep => {
                    swap(&mut self.opbuffer.ops[i].line, &mut self.lines[lineno]);
                }
            }
        }
        self.opbuffer.pending = self.opbuffer.cursor;
    }

    fn ops_undo(&mut self, op_cursor_prev: usize) {
        check!(op_cursor_prev <= self.opbuffer.cursor);
        self.opbuffer.cursor = op_cursor_prev;
        for i in self.pending_ops().rev() {
            let op_type;
            let lineno;
            let line;
            {
                let op = self.opbuffer.ops[i];
                op_type = op.op_type;
                lineno = op.lineno;
                line = op.line;
            }
            match op_type {
                Optype::Del => {
                    self.lines.insert(lineno, line);
                }
                Optype::Ins => {
                    self.lines.remove(lineno);
                }
                Optype::Rep => {
                    swap(&mut self.opbuffer.ops[i].line, &mut self.lines[lineno]);
                }
            }
        }
        self.opbuffer.pending = op_cursor_prev;
    }

    fn ops_redo(&mut self, op_cursor_next: usize) {
        check!(self.opbuffer.cursor <= op_cursor_next);
        self.opbuffer.pending = op_cursor_next;
        self.ops_do();
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertMode {
    Insert,
    Replace,
}


/*
 * Undo management and operation management
 *  - store all operations as objects in to an append only vec
 *  - snapshot are another type of operation
 *  - store sufficient information for undo and redo
 *  - when undoing, I can truncate the line buffer and the text buffer
 */

#[derive(Debug, Copy, Clone)]
struct Snapshot {
    text_cursor:    usize,
    op_cursor:      usize,
    dirty:          bool,
    cursor:         Pos,
}

// A line operation on the buffer
#[derive(Debug, Copy, Clone)]
struct Op {
    lineno:     usize,
    line:       Range,
    op_type:    Optype,
}

#[derive(Debug, Copy, Clone)]
pub enum Opresult {
    Noop,
    Cursor(Pos),
    Change(Pos),
}

#[derive(Debug, Copy, Clone)]
enum Optype {
    Del,
    Ins,
    Rep,
}

// A linear history of operations.
// OpHistory has two cursors:
//  - the current cursor represents the points of the last frame drawn
//  - the pending cursor indicates which pending ops are yet to be executed.
// When editing the buffer, pending ops are pushed in OpHistory and then batch executed.
// An OpHistory snapshot is simply a copy of the current cursor
#[derive(Debug, Clone)]
struct OpBuffer {
    ops:        Vec<Op>,
    cursor:     usize,
    pending:    usize,
}

} // mod text




/* CORE TYPE DEFINITION */

// The core editor structure
// TODO: add open buffer list, open views, open screens
struct Editor {
    window:         Pos,        // The dimensions of the editor and backend terminal window
    mainscreen:     Rec,        // The screen area for displaying file content and menus.
    footer:         Rec,
    buffer:         Buffer,     // The one file loaded in the editor
    view:           View,       // The one view of the one file loaded
    screen:         Screen,     // The one screen associated to the one file loaded
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
    Exit,
    Command,
    Insert(InsertMode),
    PendingInsert(InsertMode),
}


// Left justified, fixed length strings.
const MODE_COMMAND  : &'static str = "Command  ";
const MODE_INSERT   : &'static str = "Insert   ";
const MODE_PINSERT  : &'static str = "Insert?  ";
const MODE_REPLACE  : &'static str = "Replace  ";
const MODE_PREPLACE : &'static str = "Replace? ";
const MODE_EXIT     : &'static str = "Exit     ";

impl Mode {
    const default_command_state : Mode = Mode::Command;

    fn footer_color(self) -> Colorcell {
        use Mode::*;
        match self {
            Command                                 => CONF.color_mode_command,
            Insert(InsertMode::Insert)              => CONF.color_mode_insert,
            Insert(InsertMode::Replace)             => CONF.color_mode_replace,
            PendingInsert(InsertMode::Insert)       => CONF.color_mode_insert,
            PendingInsert(InsertMode::Replace)      => CONF.color_mode_replace,
            Exit                                    => CONF.color_mode_exit,
        }
    }

    fn name(self) -> &'static str {
        use Mode::*;
        match self {
            Command                                 => MODE_COMMAND,
            Insert(InsertMode::Insert)              => MODE_INSERT,
            Insert(InsertMode::Replace)             => MODE_REPLACE,
            PendingInsert(InsertMode::Insert)       => MODE_PINSERT,
            PendingInsert(InsertMode::Replace)      => MODE_PREPLACE,
            Exit                                    => MODE_EXIT,
        }
    }

    fn process_input(m: Mode, i: Input, e: &mut Editor) -> Re<Mode> {
        if i == Input::Key(CTRL_C) {
            // TODO: ask confirmation if dirty files
            return Ok(Exit)
        }

        if i == Input::Resize {
            logconsole("resize !");
            return Ok(m)
        }

        use Mode::*;
        let next = match m {
            Command => {
                let op = Mode::input_to_command_op(i, e);
                let next = do_command(op, e)?;
                // should this instead be managed per operation in a more scoped way ?
                e.view.update(&e.buffer);
                next
            }

            Insert(mode) => {
                let op = Mode::input_to_insert_op(mode, i, e);
                do_insert(op, e)?
            }

            PendingInsert(mode) => {
                e.buffer.snapshot(e.view.cursor);
                e.buffer.prepare_insert(usize(e.view.cursor.y));
                let insertmode = Insert(mode);
                Mode::process_input(insertmode, i, e)?
            }

            Exit => {
                panic!("cannot process input in Exit state")
            }
        };

        // FUTURE: if the cursor moved position, update all other views of that file whose cursors is below lineno

        // TODO: instead of aborting, handle input error processing, and check if any dirty files
        // need saving !

        Ok(next)
    }

    fn input_to_command_op(i: Input, e: &Editor) -> CommandOp {
        use Input::*;
        use CommandOp::*;
        match i {
            // TODO: more sophisticated cursor movement ...
            // TODO: handle mouse click
            Key('h')    => BufferMove(MoveOp::Movement(Move::Left)),
            Key('j')    => BufferMove(MoveOp::Movement(Move::Down)),
            Key('k')    => BufferMove(MoveOp::Movement(Move::Up)),
            Key('l')    => BufferMove(MoveOp::Movement(Move::Right)),
            Key(' ')    => BufferMove(MoveOp::Recenter),
            Key(CTRL_D) => BufferMove(MoveOp::PageDown),
            Key(CTRL_U) => BufferMove(MoveOp::PageUp),
            Key(CTRL_H) => BufferMove(MoveOp::FileStart),
            Key(CTRL_L) => BufferMove(MoveOp::FileEnd),
            Key('o')    => BufferOp(e.view.cursor + pos(0,1),   BufferOpType::LineNew),
            Key('O')    => BufferOp(e.view.cursor,              BufferOpType::LineNew),
            Key('q')    => BufferOp(e.view.cursor,              BufferOpType::LineJoin),
            Key(ENTER)  => BufferOp(e.view.cursor,              BufferOpType::LineBreak),
            Key('d')    => BufferOp(e.view.cursor,              BufferOpType::LineDel),
            Key('x')    => BufferOp(e.view.cursor,              BufferOpType::CharDelete),
            Key(CTRL_X) => BufferOp(e.view.cursor,              BufferOpType::CharBackspace),
            Key('u')    => BufferOp(e.view.cursor,              BufferOpType::Undo),
            Key('r')    => BufferOp(e.view.cursor,              BufferOpType::Redo),
            Key('\t')   => SwitchInsert,
            Key(CTRL_R) => SwitchReplace,
            Key('s')    => Save(format!("{}.tmp", e.view.filepath)),
            Key('\\')   => ClearConsole,
            //Key('b')
            //            => panic!("BOOM !"),
            //            //=> return er!("BOOM !"),
            _ => Noop,
        }
    }

    fn input_to_insert_op(mode: InsertMode, i: Input, e: &Editor) -> InsertOp {
        use Input::*;
        use InsertOpType::*;
        let optype = match i {
            Key(ESC) | EscZ                 => SwitchCommand,
            Key(ENTER)                      => LineBreak,
            Key(TAB)                        => TabInsert,
            Key(c) if c == DEL              => Backspace,
            Key(c) if c == BACKSPACE        => Delete,
            Key(c)                          => CharInsert(c),
            _                               => Noop,
        };

        InsertOp {
            cursor: e.view.cursor,
            optype,
            mode,
        }
    }
}

enum CommandOp {
    BufferOp(Pos, BufferOpType),
    BufferMove(MoveOp),
    Save(String),
    SwitchInsert,
    SwitchReplace,
    ClearConsole,
    Noop,
}

enum MoveOp {
    Movement(Move),
    Recenter,
    PageUp,
    PageDown,
    FileStart,
    FileEnd,
}

enum BufferOpType {
    LineDel,
    LineNew,
    LineJoin,   // TODO join line with separator !
    LineBreak,
    CharDelete,
    CharBackspace,
    Undo,
    Redo,
}

enum InsertOpType {
    LineBreak,
    TabInsert,
    CharInsert(char),
    Delete,
    Backspace,
    SwitchCommand,
    Noop,
}

struct InsertOp {
    cursor: Pos,
    optype: InsertOpType,
    mode:   InsertMode,
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

// CLEANUP: move to Cursor impl
    fn cursor_adjust(buffer: &Buffer, mut p: Pos) -> Pos {
        p.y = min(p.y, buffer.nlines() - 1);
        p.y = max(0, p.y);

        // Right bound clamp pushes x to -1 for empty lines.
        p.x = min(p.x, i32(buffer.line_len(usize(p.y))) - 1);
        p.x = max(0, p.x);

        p
    }

    // CHECKME: for cursor_next/prev, do I need to skip empty lines ?

    fn cursor_next(buffer: &Buffer, p: Pos) -> Pos {
        if p.x < i32(buffer.line_len(usize(p.y))) - 1 {
            return p + pos(1,0)
        }

        if p.y < buffer.nlines() - 1 {
            return pos(0, p.y + 1)
        }

        p // Hit end of file
    }

    fn cursor_prev(buffer: &Buffer, p: Pos) -> Pos {
        if p.x > 0 {
            return p - pos(1,0)
        }

        if p.y > 0 {
            let y = p.y - 1;
            let x = max(0, i32(buffer.line_len(usize(y))) - 1);
            return pos(x, y)
        }

        p // Hit beggining of file
    }

    fn update(&mut self, buffer: &Buffer) {
        if buffer.nlines() == 0 {
            return;
        }

        self.cursor = View::cursor_adjust(buffer, self.cursor);

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

    fn recenter(&mut self, _buffer: &Buffer) {
        let size = self.filearea.size();
        let y = max(0, self.cursor.y - size.y / 2);
        // TODO: consider horizontal recenter too
        self.filearea = pos(0, y).extrude(size);
    }

    fn go_page_down(&mut self, buffer: &Buffer) {
        let y = min(buffer.nlines() - 1, self.cursor.y + 50);
        self.cursor = pos(self.cursor.x, y);
    }

    fn go_page_up(&mut self, _buffer: &Buffer) {
        let y = max(0, self.cursor.y - 50);
        self.cursor = pos(self.cursor.x, y);
    }

    fn go_file_start(&mut self, _buffer: &Buffer) {
        self.cursor = pos(self.cursor.x, 0);
    }

    fn go_file_end(&mut self, buffer: &Buffer) {
        self.cursor = pos(self.cursor.x, buffer.nlines() - 1);
    }
}


// TODO: find better place
fn update_buffer(r: text::Opresult, e: &mut Editor) {
    use text::Opresult::*;
    match r {
        Cursor(p) => {
            e.view.cursor = p;
        }
        Change(p) => {
            e.view.cursor = p;
            e.buffer.dirty = true;
            e.buffer.ops_do();
            // push snapshot
        }
        _ => (),
    }
}

/* COMMAND AND BUFFER MANIPULATION */
    fn do_command(op: CommandOp, e: &mut Editor) -> Re<Mode> {
        use CommandOp::*;
        use Mode::*;
        match op {
            BufferMove(m) =>
                do_buffer_move(m, e),

            BufferOp(p, op) =>
                do_buffer_op(p, op, e),

            Save(path) =>
                e.buffer.to_file(&path)?,

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

        Ok(Command)
    }

    fn do_buffer_move(op: MoveOp, e: &mut Editor) {
        use MoveOp::*;
        match op {
            Movement(mvt) =>
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
        }
    }

    fn do_buffer_op(p: Pos, op: BufferOpType, e: &mut Editor) {
        use BufferOpType::*;
        let opresult = match op {
            LineDel => {
                e.buffer.snapshot(p);
                e.buffer.line_del(p)
            }

            LineNew => {
                e.buffer.snapshot(p);
                e.buffer.line_new(p)
            }

            LineJoin => {
                e.buffer.snapshot(p);
                e.buffer.line_join(p)
            }

            LineBreak => {
                e.buffer.snapshot(p);
                e.buffer.line_break(p)
            }

            CharDelete => {
                e.buffer.snapshot(p);
                e.buffer.del(p)
            }

            CharBackspace => {
                e.buffer.snapshot(p);
                e.buffer.backspace(p)
            }

            Undo => {
                e.buffer.undo()
            }

            Redo => {
                e.buffer.redo()
            }
        };

        update_buffer(opresult, e);
    }

    fn do_insert(op: InsertOp, e: &mut Editor) -> Re<Mode> {
        use InsertOpType::*;
        use text::Opresult;

        let mut next_mode = Mode::Insert(op.mode);

        let opresult = match op.optype {
            LineBreak => {
                e.buffer.line_break(op.cursor)
            }

            TabInsert => {
                let n = CONF.tab_expansion - op.cursor.x % CONF.tab_expansion;
                let mut p = op.cursor;
                for _ in 0..n {
                    e.buffer.char_insert(op.mode, p, ' ');
                    p = p + pos(1,0);
                }
                Opresult::Change(p)
            }

            CharInsert(c) if !is_printable(c) => {
                Opresult::Noop
            }

            CharInsert(c) => {
                // TODO: check that raw text mutation: insert is always preceded by a snapshot
                // and appropriate line copy
                e.buffer.char_insert(op.mode, op.cursor, c)
            }

            Delete => {
                e.buffer.del(op.cursor)
            }

            Backspace => {
                e.buffer.backspace(op.cursor)
            }

            // TODO: add SwitchReplace / SwitchInsert

            SwitchCommand => {
                next_mode = Mode::default_command_state;
                Opresult::Noop
            }

            Noop => {
                Opresult::Noop
            }
        };

        update_buffer(opresult, e);

        Ok(next_mode)
    }

impl Editor {

    fn mk_editor() -> Re<Editor> {
        let filename = file!().to_string();
        let buffer = Buffer::from_file(&filename)?;

        let window = Term::size();
        let (mainscreen, footer) = window.rec().vsplit(window.y - 1);
        let screen = Screen::mk_screen(mainscreen);
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
            buffer,
            view,
            screen,
        })
    }

    fn run() -> Re<()> {
        let mut e = Editor::mk_editor()?;
        let mut f = Framebuffer::mk_framebuffer(e.window);
        let mut m = Mode::default_command_state;

        e.refresh_screen(&mut f, &m)?;

        let (send, recv) = std::sync::mpsc::sync_channel(32);

        std::thread::spawn(move || {
            push_char(&send);
        });


        while m != Mode::Exit {
            let i = pull_input(&recv)?;
            logconsole(&format!("input: {}", i));

            let _frame_time = Scopeclock::measure("last frame");     // caveat: displayed on next frame only

            m = Mode::process_input(m, i, &mut e)?;

            e.refresh_screen(&mut f, &m)?;
        }

        Ok(())
    }

    fn refresh_screen(&mut self, framebuffer: &mut Framebuffer, mode: &Mode) -> Re<()> {
        // main screen
        {
            let _draw_time = Scopeclock::measure("draw");

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
            self.screen.put_text(framebuffer, &drawinfo);
        }

        // footer
        {
            framebuffer.put_line(self.footer.min + pos(1,0), mode.name().as_bytes());
            framebuffer.put_line(self.footer.min + pos(10, 0), b"FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER FOOTER");
            framebuffer.put_color(self.footer, mode.footer_color());
        }

        // renter frame to terminal
        {
            let _push_frame_time = Scopeclock::measure("render");

            framebuffer.render()?;
            if !CONF.retain_frame {
                framebuffer.clear();
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
