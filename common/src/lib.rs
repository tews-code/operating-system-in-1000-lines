//! Common library

#![no_std]

pub mod print;

pub const SYS_PUTBYTE: usize = 1;
pub const SYS_GETCHAR: usize = 2;
pub const SYS_EXIT: usize = 3;
pub const SYS_READFILE: usize = 4;
pub const SYS_WRITEFILE: usize = 5;
