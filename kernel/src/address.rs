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
        &raw const self.0
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
        &raw mut self.0
    }
}

pub const fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());

    (value + (align - 1)) & !(align - 1)
}
