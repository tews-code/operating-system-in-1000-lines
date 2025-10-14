# Address Module

In this chapter, let's implement basic address types and memory operations.

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

    pub const fn set(&mut self, addr: usize) {
        self.0 = addr;
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_ptr(self) -> *const usize {
        self.0 as *const usize
    }
}

// Virtual Address
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct VAddr(usize);

pub const fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());

    (value + (align - 1)) & !(align - 1)
}
```

We create a `struct PAddr` which has a single tuple `usize`. This is has a memory representation of `repr(transparent)`, meaning it is exactly the memory representation of a `usize`. We then create a set of simple helper functions, which are `const` so we can use them in a const or static environment.

We also create an equivalent `struct VAddr`, but here we have not bothered to create the helper functions as we will not need them in our simple operating system.

Finally, we create an `align_up` function. We use the `debug_assert!` macro to catch any attempts to align to a non power of two (but this would be optimised out of release code).
