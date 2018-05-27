#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]




use std::fs;
use std::io;
use std::io::prelude::*;
use std::str;
use std::result;



/*
 * Next Steps:
 *
 *      - add input event parser, print keys on screen
 *      - load a file a draw that file
 *      - add raw term
 *      - add basic cursor navigation
 *      - add insert text
 */





/* CORE TYPE DEFINITION */

// The core editor structure
struct Editor {
    window:         Vek,              // The dimensions of the editor and backend terminal window
    framebuffer:    Framebuffer,

    running: bool,
    // TODO:
    //  list of open files and their filebuffers
    //  list of screens
    //  current screen layout
    //  Mode state machine
}

struct Config {
    // TODO
}

// Either a position in 2d space w.r.t to (0,0), or a movement quantity
#[derive(Debug, Clone, Copy)]
struct Vek { // Vec was already taken ...
    x: i32,
    y: i32,
}

// A simple rectangle
#[derive(Debug, Clone, Copy)]
struct Rec {
    min: Vek,   // point the closest to (0,0)
    max: Vek,   // point the farthest to (0,0)
}

type Colorcode = i32;

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
    /* High contract 8 ansi colors */
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

enum Move {
    Left,
    Right,
    Up,
    Down,
    Start,
    End,
}

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

// The struct that manages compositing.
struct Framebuffer {
    window:     Vek,
    len:        i32,

    text:       Vec<u8>,
    fg:         Vec<u8>,
    bg:         Vec<u8>,
    cursor:     Vek,

    buffer:     Bytebuffer,
}

// Append only buffer with a cursor
struct Bytebuffer {
    bytes: Vec<u8>,
    cursor: i32,
}

// Transient object for putting text into a subrectangle of a framebuffer.
// Since it needs a mut ref to the framebuffer, Screen objs cannot be stored.
struct Screen<'a> {
    framebuffer:    &'a mut Framebuffer,
    window:         Rec,
}

// Manage content of a file
struct Filebuffer {
    // TODO
}

// Point to a place inside a Filebuffer
struct Cursor<'a> {
    filebuffer: &'a Filebuffer,
}

// Store states related to navigation in a given file.
struct Fileview {
    relative_lineno: bool,
    movement_mode: MovementMode,
    show_token: bool,
    show_neighbor: bool,
    show_selection: bool,
    //selection:  Option<&[Selection]>
}


// + everything needed for input processing ...




/* CORE TYPES IMPLS */

fn vek(x: i32, y: i32) -> Vek {
    return Vek {
       x,
       y,
    };
}

// TODO: rec ctor with width and height ??
fn rec(x0: i32, y0: i32, x1: i32, y1: i32) -> Rec {
    let (a0, a1) = ordered(x0, x1);
    let (b0, b1) = ordered(x0, x1);
    return Rec {
        min: vek(a0, b0),
        max: vek(a1, b1),
    };
}


impl Rec {
    fn x0(self) -> i32 { return self.min.x; }
    fn y0(self) -> i32 { return self.min.y; }
    fn x1(self) -> i32 { return self.max.x; }
    fn y1(self) -> i32 { return self.max.y; }
    fn w(self) -> i32 { return self.max.x - self.min.x; }
    fn h(self) -> i32 { return self.max.y - self.min.y; }

    fn area(self) -> i32 { return self.w() * self.h(); }
    fn size(self) -> Vek { return vek(self.w(), self.h()); }
}


/* Vek/Vek ops */

impl Vek {
    fn rec(self) -> Rec {
        return Rec {
            min: vek(0,0),
            max: self,
        };
    }
}

impl std::ops::Add<Vek> for Vek {
    type Output = Vek;

    fn add(self, v: Vek) -> Vek {
        return vek(self.x + v.x, self.y + v.y);
    }
}

impl std::ops::Sub<Vek> for Vek {
    type Output = Vek;

    fn sub(self, v: Vek) -> Vek {
        return vek(self.x - v.x, self.y - v.y);
    }
}

impl std::ops::Neg for Vek {
    type Output = Vek;

    fn neg(self) -> Vek {
        return vek(-self.x, -self.y);
    }
}

/* Vek/Rec ops */

impl Rec {
    fn is_inside(self, v : Vek) -> bool {
        return self.min.x <= v.x
            && self.min.y <= v.y
            &&               v.x <= self.max.x
            &&               v.y <= self.max.y
    }
}

impl std::ops::Add<Vek> for Rec {
    type Output = Rec;

    fn add(self, v: Vek) -> Rec {
        return Rec {
            min: self.min + v,
            max: self.max + v,
        };
    }
}

impl std::ops::Add<Rec> for Vek {
    type Output = Rec;

    fn add(self, r: Rec) -> Rec {
        return r + self;
    }
}

impl std::ops::Sub<Vek> for Rec {
    type Output = Rec;

    fn sub(self, v: Vek) -> Rec {
        return Rec {
            min: self.min - v,
            max: self.max - v,
        };
    }
}


/* Colors */

fn colorcode(c : Color) -> Colorcode {
    match c {
        // TODO !
        Color::Black                    => 0,
        Color::Red                      => 0,
        Color::Green                    => 0,
        Color::Yellow                   => 0,
        Color::Blue                     => 0,
        Color::Magenta                  => 0,
        Color::Cyan                     => 0,
        Color::White                    => 0,
        Color::BoldBlack                => 0,
        Color::BoldRed                  => 0,
        Color::BoldGreen                => 0,
        Color::BoldYellow               => 0,
        Color::BoldBlue                 => 0,
        Color::BoldMagenta              => 0,
        Color::BoldCyan                 => 0,
        Color::BoldWhite                => 0,
        Color::RGB216 { r, g, b }       => 0,
        Color::Gray(g)                  => 0,
    }
}


/* Utils */

fn ordered<T>(v1: T, v2: T) -> (T, T) where T : Ord {
    if v1 < v2 {
        return (v1, v2);
    }
    return (v2, v1);
}

fn reorder<T>(v1: &mut T, v2: &mut T) where T : Ord {
    if v1 < v2 {
        return;
    }
    std::mem::swap(v1, v2);
}



impl Editor {

    fn init() /*-> Editor */ {
        // TODO
    }

    fn run(&mut self) {
        while self.running {
            self.refresh_screen();
            self.proces_input();
        }
    }

    fn refresh_screen(&mut self) {
        // TODO
    }

    fn proces_input(&mut self) {
        // TODO
    }

    fn resize(&mut self) {
        // TODO
    }
}







type Rez<T> = result::Result<T, String>;

// TODO: associate this to a Filebuffer struct
// TODO: probably I need to collapse all errors into strings, and create my own Result alias ...
fn file_load(filename: &str) -> io::Result<Vec<u8>>
{
    let fileinfo = try!(fs::metadata(filename));
    let size = fileinfo.len() as usize;

    let mut buf = vec![0; size];
    let mut f = try!(fs::File::open(filename));

    let nread = try!(f.read(&mut buf));
    if nread != size {
        // why so ugly ...
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "not read enough bytes")); // TODO: add number of bytes
    }

    return Ok(buf);
}

fn file_lines_print(buf: &[u8])
{
    let newline = '\n' as u8;
    for (i, line) in buf.split(|c| *c == newline).enumerate() {
        println!("{}: {}", i, str::from_utf8(line).unwrap());
    }
}

fn main()
{
    let filename = "./term.rs";

    let buf = file_load(filename).unwrap();

    file_lines_print(&buf);
}
