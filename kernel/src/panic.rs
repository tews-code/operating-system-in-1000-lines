//! Panic for os1k

use core::arch::asm;
use core::panic::PanicInfo;

use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("⚠️ Panic: {}", info);

    loop {
        unsafe {asm!("wfi")};
    }
}
