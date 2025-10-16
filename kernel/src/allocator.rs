//! Allocate memory pages

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::write_bytes;

use crate::address::{align_up, PAddr};
use crate::spinlock::SpinLock;

const PAGE_SIZE: usize = 4096;

//Safety: Symbols created by linker script
unsafe extern "C" {
    static __free_ram: u8;
    static __free_ram_end: u8;
}

#[derive(Debug)]
struct BumpAllocator(SpinLock<Option<PAddr>>);

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator(
    SpinLock::new(None),
);

unsafe impl GlobalAlloc for BumpAllocator {
    // Safety: Caller must ensure that Layout has a non-zero size
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_assert!(layout.size() > 0, "allocation size must be non-zero");

        let mut next_paddr = self.0.lock();

        // Initialise on first use
        if next_paddr.is_none() {
            *next_paddr = Some(PAddr::new(&raw const __free_ram as usize))
        }

        // Safe to unwrap as we know it is Some now
        let paddr = next_paddr.unwrap();
        let new_paddr = paddr.as_usize() + align_up(layout.size(), PAGE_SIZE);
        if new_paddr > &raw const __free_ram_end as usize {
            panic!("out of memory");
        }
        *next_paddr = Some(PAddr::new(new_paddr));

        // Safety: paddr.as_ptr() is aligned and not null; entire PAGE_SIZE of bytes is available for write
        unsafe{ write_bytes(paddr.as_ptr() as *mut usize, 0, PAGE_SIZE) };

        common::println!("alloc_pages test: {:x}", paddr.as_usize());

        paddr.as_ptr() as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
