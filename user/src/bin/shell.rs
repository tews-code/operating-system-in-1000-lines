//! os1k shell

#![no_std]
#![no_main]

use user::println;

#[unsafe(no_mangle)]
fn main() {
    println!("Hello world from the shell!");

    loop {}
}
