#![allow(dead_code)]
#![allow(non_upper_case_globals)]

use std::{env, fs};
use std::ffi::OsStr;
//use std::collections::HashMap;
use std::vec::Vec;
use std::fs::ReadDir;

fn main() -> Result<(), std::io::Error> {
    let current_dir = env::current_dir()?;
    let mut vec : std::vec::Vec<ReadDir> = Vec::new();
    vec.push(fs::read_dir(current_dir)?);

    while !vec.is_empty() {
        for entry in vec.pop().unwrap() {
            match entry {
                Err(_) => (),
                Ok(entry) => {
                    let path = entry.path();
                    let metadata = fs::metadata(&path)?;
                    //if metadata.is_file() {
                    //}
//                    let x = path.file_name().map(&OsStr::to_str).flatten();
//                    match x {
//                        Some(n) => println!("{}", n),
//                        _ => ()
//                    }

//                    let mut sep = " ";
//                    for comp in path.components() {
//                        //match comp {
//                        //}
//                        match comp.as_os_str().to_str() {
//                            Some(n) => print!("{}{}", sep, n),
//                            _ => ()
//                        }
//                        sep = " ";
//                    }
//                    println!();
                    if metadata.is_dir() {
                        vec.push(fs::read_dir(path)?);
                    }
                }
            }
        }
    }

    Ok(())
}
