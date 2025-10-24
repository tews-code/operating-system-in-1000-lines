//! os1k shell

#![no_std]
#![no_main]

use user::{
    print,
    println,
    get_char,
    put_byte,
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
            _ => {
                println!("unknown command: {}", cmdline_str);
            },
        }
    }
}
