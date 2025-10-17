//! OS in 1000 lines

#![no_std]
#![no_main]

pub extern crate alloc;

use core::arch::{asm, naked_asm};
use core::ptr::write_bytes;

#[allow(unused_imports)]
use common::{print, println};

mod address;
mod allocator;
#[macro_use]
mod entry;
mod panic;
mod process;
mod sbi;
mod scheduler;
mod spinlock;

use crate::entry::kernel_entry;
use crate::process::create_process;
use crate::scheduler::yield_now;
use crate::spinlock::SpinLock;


// Safety: Symbols created by linker script
unsafe extern "C" {
    static __bss: u8;
    static __bss_end: u8;
    static __stack_top: u8;
}

fn delay() {
    for _ in 0..300_000_000usize {
        unsafe{asm!("nop");} // do nothing
    }
}

static PROC_A: SpinLock<Option<usize>> = SpinLock::new(None);
static PROC_B: SpinLock<Option<usize>> = SpinLock::new(None);

fn proc_a_entry() {
    println!("starting process A");
    loop {
        print!("🐈");
        yield_now();
        delay()
    }
}

fn proc_b_entry() {
    println!("starting process B");
    loop {
        print!("🐕");
        yield_now();
        delay()
    }
}


#[unsafe(no_mangle)]
fn kernel_main() -> ! {
    let bss = &raw const __bss;
    let bss_end = &raw const __bss_end;
    // Safety: from linker script bss is aligned and bss segment is valid for writes up to bss_end
    unsafe {
        write_bytes(bss as *mut u8, 0, bss_end as usize - bss as usize);
    }

    write_csr!("stvec", kernel_entry as usize);

    common::println!("Hello World! 🦀");

    PROC_A.lock().get_or_insert_with(|| {
        create_process(proc_a_entry as usize)
    });
    PROC_B.lock().get_or_insert_with(|| {
        create_process(proc_b_entry as usize)
    });

    yield_now();

    panic!("switched to idle process");
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn boot() -> ! {
    naked_asm!(
        "la a0, {stack_top}",
        "mv sp, a0",
        "j {kernel_main}",
        stack_top = sym __stack_top,
        kernel_main = sym kernel_main,
    );
}
