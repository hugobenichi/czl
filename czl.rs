#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]




use std::fs;
use std::io;
use std::io::prelude::*;
use std::str;
use std::result;



// Main variable holding all the editor data.
static E : Editor = Editor {
    fb: Framebuffer {
        text: [0; 4096],
    },
    size: Vek {
        x: 0,
        y: 0,
    },
    bb: Bytebuffer {
        cursor: 0,
        data: [0; 512 * 512],
    },
};


/* CORE TYPE DEFINITION */

// The core editor structure
struct Editor {
    size: Vek,              // The dimensions of the editor and backend terminal window
    fb: Framebuffer,
    bb: Bytebuffer,
}


// The struct that manages compositing.
struct Framebuffer {
    text: [i32; 4096],
}

// Fixed size append buffer used by Framebuffer to send frame data to the terminal.
struct Bytebuffer {
    cursor: i32,
    data: [u8; 512 * 512],
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
    fn a(self) -> i32 { return self.w() * self.h(); }
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


/* utils */

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
