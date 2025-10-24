//! Kernel entry

use core::arch::naked_asm;

use common::{
    SYS_PUTBYTE,
    SYS_GETCHAR,
    SYS_EXIT
};

use crate::process::{PROCS, State};
use crate::sbi::{put_byte, get_char};
use crate::scheduler::{yield_now, CURRENT_PROC};
use crate::{read_csr, write_csr};

const SCAUSE_ECALL: usize = 8;

#[repr(C, packed)]
struct TrapFrame{
    ra: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
    sp: usize,
}

#[unsafe(naked)]
pub unsafe extern "C" fn kernel_entry() {
    naked_asm!(
        ".align 2",
        // Retrieve the kernel stack of the running process from sscratch.
        "csrrw sp, sscratch, sp",
        "addi sp, sp, -4 * 31",
        "sw ra,  4 * 0(sp)",
        "sw gp,  4 * 1(sp)",
        "sw tp,  4 * 2(sp)",
        "sw t0,  4 * 3(sp)",
        "sw t1,  4 * 4(sp)",
        "sw t2,  4 * 5(sp)",
        "sw t3,  4 * 6(sp)",
        "sw t4,  4 * 7(sp)",
        "sw t5,  4 * 8(sp)",
        "sw t6,  4 * 9(sp)",
        "sw a0,  4 * 10(sp)",
        "sw a1,  4 * 11(sp)",
        "sw a2,  4 * 12(sp)",
        "sw a3,  4 * 13(sp)",
        "sw a4,  4 * 14(sp)",
        "sw a5,  4 * 15(sp)",
        "sw a6,  4 * 16(sp)",
        "sw a7,  4 * 17(sp)",
        "sw s0,  4 * 18(sp)",
        "sw s1,  4 * 19(sp)",
        "sw s2,  4 * 20(sp)",
        "sw s3,  4 * 21(sp)",
        "sw s4,  4 * 22(sp)",
        "sw s5,  4 * 23(sp)",
        "sw s6,  4 * 24(sp)",
        "sw s7,  4 * 25(sp)",
        "sw s8,  4 * 26(sp)",
        "sw s9,  4 * 27(sp)",
        "sw s10, 4 * 28(sp)",
        "sw s11, 4 * 29(sp)",

        // Retrieve and save the sp at the time of exeception
        "csrr a0, sscratch",
        "sw a0, 4 * 30(sp)",

        // Reset the kernel stack.
        "addi a0, sp, 4 * 31",
        "csrw sscratch, a0",

        "mv a0, sp",
        "call handle_trap",

        "lw ra,  4 * 0(sp)",
        "lw gp,  4 * 1(sp)",
        "lw tp,  4 * 2(sp)",
        "lw t0,  4 * 3(sp)",
        "lw t1,  4 * 4(sp)",
        "lw t2,  4 * 5(sp)",
        "lw t3,  4 * 6(sp)",
        "lw t4,  4 * 7(sp)",
        "lw t5,  4 * 8(sp)",
        "lw t6,  4 * 9(sp)",
        "lw a0,  4 * 10(sp)",
        "lw a1,  4 * 11(sp)",
        "lw a2,  4 * 12(sp)",
        "lw a3,  4 * 13(sp)",
        "lw a4,  4 * 14(sp)",
        "lw a5,  4 * 15(sp)",
        "lw a6,  4 * 16(sp)",
        "lw a7,  4 * 17(sp)",
        "lw s0,  4 * 18(sp)",
        "lw s1,  4 * 19(sp)",
        "lw s2,  4 * 20(sp)",
        "lw s3,  4 * 21(sp)",
        "lw s4,  4 * 22(sp)",
        "lw s5,  4 * 23(sp)",
        "lw s6,  4 * 24(sp)",
        "lw s7,  4 * 25(sp)",
        "lw s8,  4 * 26(sp)",
        "lw s9,  4 * 27(sp)",
        "lw s10, 4 * 28(sp)",
        "lw s11, 4 * 29(sp)",
        "lw sp,  4 * 30(sp)",
        "sret"
    )
}

#[unsafe(no_mangle)]
extern "C" fn handle_trap(f: &mut TrapFrame) {
    let scause = read_csr!("scause");
    let stval = read_csr!("stval");
    let mut user_pc = read_csr!("sepc");

    if scause == SCAUSE_ECALL {
        handle_syscall(f);
        user_pc += 4;
    } else {
            panic!("unexpected trap scause=0x{:x}, stval=0x{:x}, sepc=0x{:x}", scause, stval, user_pc);
    }

    write_csr!("sepc", user_pc);
}

fn handle_syscall(f: &mut TrapFrame) {
    let sysno = f.a4;
    match sysno {
        SYS_PUTBYTE => {  // Match what user code sends
            match put_byte(f.a0 as u8) {
                Ok(_) => f.a0 = 0,     // Set return value to 0 (success)
                Err(e) => f.a0 = e as usize,    // Set return value to error code
            }
        },
        SYS_GETCHAR => {
            loop {
                if let Ok(ch) = get_char() {
                    f.a0 = ch as usize;
                    break;
                }
                yield_now();
            }
        },
        SYS_EXIT => {
            let current = CURRENT_PROC.lock()
                .expect("current process should be running");
            crate::println!("process {} exited", current);
            if let Some(p) = PROCS.0.lock().iter_mut()
                .find(|p| p.pid == current) {
                    p.state = State::Exited
                }
            yield_now();
            unreachable!("unreachable after SYS_EXIT");
        }
        _ => {panic!("unexpected syscall sysno={:x}", sysno);},
    }
}

#[macro_export]
macro_rules! read_csr {
    ( $reg:literal ) => {
        {
            let val: usize;
            unsafe{core::arch::asm!(concat!("csrr {}, ", $reg), out(reg) val)}
            val
        }
    };
}

#[macro_export]
macro_rules! write_csr {
    ( $reg:literal, $val:expr ) => {
        {
            let val = $val; // Expand metavariable outside of unsafe block (avoids clippy warning)
            unsafe{core::arch::asm!(concat!("csrw ", $reg, ", {}"), in(reg) val)}
        }
    };
}
