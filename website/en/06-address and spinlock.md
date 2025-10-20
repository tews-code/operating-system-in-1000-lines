# Address Module

In this chapter, let's implement basic address types a spin lock.

> [!TIP]
>
> The concepts introduced in this chapter are very common in Rust programming, so ChatGPT would provide solid answers. If you struggle with implementation or understanding any part, feel free to try asking it or ping me.

## Address types

First, let's create a new module file `address.rs` and add `mod address` to `main.rs`. Then let's define a Physical Address zero-size-type, as well as a Virtual Address ZST. These will prevent us from accidentally using the wrong memory address types in our code - it simply will not compile!

```rust [kernel/src/address.rs] 
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
        &raw mut self.0 as *mut usize
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
```

We create a `struct PAddr` which has a single tuple `usize`. This is has a memory representation of `repr(transparent)`, meaning it is exactly the memory representation of a `usize`. We then create a set of simple helper methods, which are `const` so we can use them in a const or static environment.

We also create an equivalent `struct VAddr`, with an additional helper method `field_raw_ptr` that provides a pointer to the VAddr field.

 We create an `align_up` function. We use the `debug_assert!` macro to catch any attempts to align to a non power of two (but this would be excluded from `release` code). 

`align_up` is useful when dealing with memory alignment. For example, align_up(0x1234, 0x1000) returns 0x2000.

We also have a function `is_aligned` which will tell us if a `usize` is aligned to any particular power of two.

# Spin lock

We will create a spin lock to protect shared global resources. Create a new file `spinlock.rs` and add this as a module in `main.rs`.

> [!TIP]
>
> The spin lock is taken directly from the excellent [Rust Atomics and Locks](https://marabos.nl/atomics/building-spinlock.html) by Mara Bos.

```rust [kernel/src/spinlock.rs]
//! Spinlock for os1k

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering::{Acquire, Release}};

#[derive(Debug)]
pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for SpinLock<T> where T: Send {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> Guard<'_, T> {
        while self.locked.swap(true, Acquire) {
            core::hint::spin_loop();
        }
        Guard { lock: self }
    }
}

#[derive(Debug)]
pub struct Guard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        //Safety: The existance of this guard guarantees exclusive lock
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        //Safety: The existance of this guard guarantees exclusive lock
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Release);
    }
}
```

The spinlock allows us to avoid using `static mut` global variables. Instead, we wrap shared global variables in our `SpinLock`, which has an atomic `AtomicBool` that indicates whether the it is locked, and an `UnsafeCell` for interior mutability. 

To lock the spinlock, we set the boolean to `true` with _load_/_acquire_ memory ordering, and unlock by setting it to `false` using _store_/_release_ ordering. This creates a "happens before" relationship between taking a new lock and the previously held lock. 

To make sure we are exclusively changing the locked value, we use a "guard" which holds the spinlock, and which we can only get by successfully locking.

Working with the guard directly means awkwardly dereferencing to get the lock, so we implement the `Deref` and `DerefMut` traits on it (which will perform an automatic deferencing), allowing us to treat the guard as if it were the lock. 

We also implement `Drop` on the guard to unlock the lock. That avoids us having to explicitly drop the lock (although we can do so if we want to using `drop(lock_name)`.
