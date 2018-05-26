#![allow(dead_code)]
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
    size: Point {
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
    size: Point,              // The dimensions of the editor and backend terminal window
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

struct Point {
  x: i32,
  y: i32,
}

// Either a position in 2d space w.r.t to (0,0), or a movement quantity
impl Point {
  fn add(self, v: Point) -> Point {
    return vec(self.x + v.x, self.y + v.y);
  }
}

fn vec(x: i32, y: i32) -> Point {
  return Point {
    x,
    y,
  };
}

type Rez<T> = result::Result<T, String>;

// TODO: associate this to a Filebuffer struct
// TODO: probably I need to collapse all errors into strings, and create my own Result alias ...
fn file_load(filename: &str) -> io::Result<Vec<u8>>
{
    let fileinfo = try!(fs::metadata(filename));
    let size = fileinfo.len() as usize;

    let mut buf : Vec<u8> = vec![0; size];
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
