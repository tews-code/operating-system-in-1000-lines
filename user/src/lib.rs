//! User library for os1k

#![no_std]

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

pub use common::{print, println};

use common::{
    SYS_PUTBYTE,
    SYS_GETCHAR,
    SYS_EXIT,
    SYS_READFILE,
    SYS_WRITEFILE,
};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("😬 User Panic! {}", info);
    exit();
}

unsafe extern "C" {
    static __user_stack_top: u8;
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

#[unsafe(no_mangle)]
pub fn exit() -> ! {
    let _ = sys_call(SYS_EXIT, 0, 0, 0, 0);
    unreachable!("just in case!");
}

pub fn readfile(filename: &str, buf: &mut [u8]) {
    let _ = sys_call(SYS_READFILE, filename.as_ptr() as isize, filename.len() as isize, buf.as_mut_ptr() as isize, buf.len() as isize);
}

pub fn writefile(filename: &str, buf: &[u8]) {
    let _ = sys_call(SYS_WRITEFILE, filename.as_ptr() as isize, filename.len() as isize,  buf.as_ptr() as isize, buf.len() as isize);
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

