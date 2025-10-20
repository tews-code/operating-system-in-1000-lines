//! User library for os1k

#![no_std]

use core::arch::naked_asm;
use core::panic::PanicInfo;

pub use common::{print, println};

// pub mod syscall;

#[panic_handler]
pub fn panic(_panic: &PanicInfo) -> ! {
    loop {}
}

unsafe extern "C" {
    static __user_stack_top: u8;
}

#[unsafe(no_mangle)]
fn exit() -> ! {
    loop {}
}

#[unsafe(link_section = ".text.start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn start() {
    naked_asm!(
        "la sp, {stack_top}",
        "call main",
        "call exit",
        stack_top = sym __user_stack_top
    )
}
