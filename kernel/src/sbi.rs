//! SBI Interface

use core::arch::asm;
use core::ffi::{c_long, c_int};

pub const EID_CONSOLE_PUTCHAR: c_long = 1;
pub const EID_CONSOLE_GETCHAR: c_long = 2;


// Safety: Caller must ensure that SBI call does not change machine state, memory mappings etc.
pub unsafe fn sbi_call(mut arg0: c_int, eid: c_long) -> Result<isize, isize> {
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0,
            in("a7") eid,
        );
    }

    match eid {
        EID_CONSOLE_PUTCHAR => {
            if arg0 == 0 {
                Ok(0)
            } else {
                Err(arg0 as isize)
            }
        },
        EID_CONSOLE_GETCHAR => {
            if arg0 != -1 {
                Ok(arg0 as isize)
            } else {
                Err(-1)
            }
        },
        _ => {
            panic!("Unknown SBI EID");
        }
    }
}

#[unsafe(no_mangle)]
pub fn put_byte(b: u8) -> Result<isize, isize> {
    // Safety: EID_CONSOLE_PUTCHAR is a safe SBI call that only writes to console
    unsafe {
        sbi_call(b as c_int, EID_CONSOLE_PUTCHAR)
    }
}

pub fn get_char() -> Result<isize, isize> {
    let ret = unsafe {
        sbi_call(0, EID_CONSOLE_GETCHAR)
    };
    ret
}
