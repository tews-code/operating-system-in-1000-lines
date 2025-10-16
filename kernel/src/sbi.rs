//! SBI Interface

use core::arch::asm;
use core::ffi::{c_long, c_int};

pub const EID_CONSOLE_PUTCHAR: c_long = 1;

// Safety: Caller must ensure that SBI call does not change machine state, memory mappings etc.
pub unsafe fn sbi_call(arg0: c_int, eid: c_long) -> Result<isize, isize> {
    let result: c_long;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0 => result,
            in("a7") eid,
        );
    }
    match result {
        0 => Ok(0),
        _ => Err(result as isize),
    }
}

#[unsafe(no_mangle)]
pub fn put_byte(b: u8) -> Result<isize, isize> {
    // Safety: EID_CONSOLE_PUTCHAR is a safe SBI call that only writes to console
    unsafe {
        sbi_call(b as c_int, EID_CONSOLE_PUTCHAR)
    }
}
