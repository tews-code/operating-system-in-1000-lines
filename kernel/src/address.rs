//! Address functions for os1k

#![allow(dead_code)]

// Physical Address
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PAddr(usize);

impl PAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub fn set(&mut self, addr: usize) {
        self.0 = addr;
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn as_ptr(&self) -> *const usize {
        self.0 as *const usize
    }

    pub const fn as_ptr_mut(&mut self) -> *mut usize {
         self.0 as *mut usize
    }
}

// Virtual Address
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn as_ptr_mut(&mut self) -> *mut usize {
        self.0 as *mut usize
    }

    pub const fn field_raw_ptr(&mut self) -> *mut usize {
        &raw mut self.0
    }
}

pub const fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());

    (value + (align - 1)) & !(align - 1)
}

pub const fn is_aligned(value: usize, align: usize) -> bool {
    assert!(align.is_power_of_two(), "align must be a power of 2");
    let align_mask = align - 1;
    value & align_mask == 0
}
