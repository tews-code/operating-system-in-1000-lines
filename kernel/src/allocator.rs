//! Allocate memory pages

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::write_bytes;

use crate::address::{align_up, PAddr};
use crate::spinlock::SpinLock;

pub const PAGE_SIZE: usize = 4096;

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
        let mut paddr = *next_paddr.get_or_insert_with(|| {
            PAddr::new(&raw const __free_ram as usize)
        });

        let aligned_size = align_up(layout.size(), PAGE_SIZE);

        let new_paddr = paddr.as_usize() + aligned_size;
        if new_paddr > &raw const __free_ram_end as usize {
            panic!("out of memory");
        }

        *next_paddr = Some(PAddr::new(new_paddr));

        // Safety: paddr.as_ptr_mut() is aligned and not null; entire aligned_size of bytes is available for write
        unsafe{ write_bytes(paddr.as_ptr_mut() as *mut u8, 0x55, aligned_size) };

        // crate::println!("alloc page: {:x}", paddr.as_usize());
        // for _ in 0..5 {
        //     crate::delay();
        // }

        paddr.as_ptr() as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
