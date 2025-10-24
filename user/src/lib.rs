//! User library for os1k

#![no_std]

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

pub use common::{print, println};

use common::{
    SYS_PUTBYTE,
    SYS_GETCHAR,
};

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

pub fn sys_call(sysno: usize, arg0: isize, arg1: isize, arg2: isize, arg3: isize) -> isize {
    let a0: isize;
    unsafe{asm!(
        "ecall",
        inout("a0") arg0 => a0,
        in("a1") arg1,
        in("a2") arg2,
        in("a3") arg3,
        in("a4") sysno,
    )}
    a0
}

#[unsafe(no_mangle)]
pub fn put_byte(b: u8) -> Result<(), isize> {
    let result = sys_call(SYS_PUTBYTE, b as isize, 0, 0, 0);
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

pub fn get_char() -> Option<usize> {
    let ch = sys_call(SYS_GETCHAR, 0, 0, 0, 0);
    if ch == -1 {
        None
    } else {
        Some(ch as usize)
    }
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
