use std::fs;
use std::io::prelude::*;
use std::str;

fn main() {
    let filename = "./term.rs";

    println!("In file {}", filename);

    // TODO: cleanly unwrap
    let fileinfo = fs::metadata(filename).expect("file not found");
    let size = fileinfo.len() as usize;

    let mut buf = vec![0; size];
    let mut f = fs::File::open(filename).expect("file not found");

    let nread = f.read(&mut buf);
    match nread {
        Ok(n) if n == size  => (),
        _                   => panic!("io error"),
    }

    let newline = '\n' as u8;
    for (i, line) in buf.split_mut(|c| *c == newline).enumerate() {
        println!("{}: {}", i, str::from_utf8(line).unwrap());
    }
}
