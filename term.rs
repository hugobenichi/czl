use std::fs;
use std::io;
use std::io::prelude::*;
use std::str;


// TODO: probably I need to collapse all errors into strings, and create my own Result alias ...
fn file_load(filename: &str) -> std::io::Result<Vec<u8>>
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
