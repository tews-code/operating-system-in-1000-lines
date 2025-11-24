//! RISC-V Sv32 Page Table

use alloc::boxed::Box;
use core::ops::{Index, IndexMut};

use crate::address::{is_aligned, PAddr, VAddr};
use crate::allocator::PAGE_SIZE;

const ENTRIES_PER_TABLE: usize = 1024; // Each Page Table Entry is 4 bytes in Sv32

pub const SATP_SV32: usize = 1 << 31;
pub const PAGE_V: usize = 1 << 0;   // "Valid" bit (entry is enabled)
pub const PAGE_R: usize = 1 << 1;   // Readable
pub const PAGE_W: usize = 1 << 2;   // Writable
pub const PAGE_X: usize = 1 << 3;   // Executable
pub const PAGE_U: usize = 1 << 4;   // User (accessible in user mode)

impl VAddr {
    fn vpn0(&self) -> usize {
        self.as_usize() >> 12 & 0x3FF
    }

    fn vpn1(&self) -> usize {
        self.as_usize() >> 22 & 0x3FF
    }
}

impl PAddr {
    fn ppn(&self) -> usize {
        (self.as_usize() / PAGE_SIZE) << 10
    }

    fn from_ppn(pte: usize) -> Self {
        PAddr::new((pte >> 10) * PAGE_SIZE)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PageTable([usize; ENTRIES_PER_TABLE]);

impl PageTable {
    pub const fn new() -> Self {
        Self([0; ENTRIES_PER_TABLE])
    }
}

impl Index<usize> for PageTable {
    type Output = usize;

    fn index(&self, vpn: usize) -> &Self::Output {
        &self.0[vpn]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, vpn: usize) -> &mut Self::Output {
        &mut self.0[vpn]
    }
}

pub fn map_page(table1: &mut PageTable, vaddr: VAddr, paddr: PAddr, flags: usize) {
    assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE), "unaligned vaddr {}", vaddr.as_usize());
    assert!(is_aligned(paddr.as_usize(), PAGE_SIZE), "unaligned paddr {}", paddr.as_usize());

    let vpn1 = vaddr.vpn1();

    // Create the 1st level page table if it doesn't exist.
    if table1[vpn1] & PAGE_V == 0 {
        let table0 = Box::new(PageTable::new());
        let table0_paddr = PAddr::new(Box::into_raw(table0) as *mut _ as usize);
        table1[vpn1] = table0_paddr.ppn() | PAGE_V;
    }

    let table0 = unsafe {
        let mut table0_paddr = PAddr::from_ppn(table1[vpn1]);
        &mut *(table0_paddr.as_ptr_mut() as *mut PageTable)
    };

    table0[vaddr.vpn0()] = paddr.ppn() | flags | PAGE_V;
}

