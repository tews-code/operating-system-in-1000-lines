//! os1k shell

#![no_std]
#![no_main]

use core::ffi::CStr;

use user::{
    exit,
    print,
    println,
    get_char,
    put_byte,
    readfile,
    writefile,
};

#[unsafe(no_mangle)]
fn main() {
    loop {
        print!("> ");
        let mut cmdline = [b'\n'; 128];
        let mut pos = 0;
        loop {
            let Some(ch) = get_char() else {
                break;
            };
            let byte = ch as u8;
            match byte {
                b'\r' => { // On the debug console the newline is \r
                    println!();
                    break;
                },
                _ => {
                    let _ = put_byte(byte);
                    cmdline[pos] = byte;
                    pos += 1;
                }
            }
        }

        let cmdline_str = str::from_utf8(&cmdline)
        .expect("command line text valid UTF8")
        .trim();

        match cmdline_str {
            "hello" => {
                println!("Hello world from the shell! 🐚");
            },
            "exit" => {
                exit();
            },
            "readfile" => {
                let mut buf = [0u8; 128];
                readfile("hello.txt", &mut buf);
                CStr::from_bytes_until_nul(&buf)
                .ok()
                .and_then(|cstr| cstr.to_str().ok())
                .map(|s| println!("{}", s.trim_end()))
                .unwrap_or_else(|| println!("could not read file contents"));
            }
            "writefile" => {
                writefile(
                    "meow.txt",
                    b"Hello from the shell!");
            },
            _ => {
                println!("unknown command: {}", cmdline_str);
            },
        }
    }
}
