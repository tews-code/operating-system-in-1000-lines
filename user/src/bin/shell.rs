//! os1k shell

#![no_std]
#![no_main]

use core::arch::asm;

#[expect(unused_imports)]
use user;

#[unsafe(no_mangle)]
fn main() {
    loop {}
}
