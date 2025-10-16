//! Print to debug console

use core::fmt;

pub struct DebugConsole;

unsafe extern "Rust" {
    pub fn put_byte(b: u8) -> Result<isize, isize>;
}

impl fmt::Write for DebugConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            unsafe { put_byte(b).map_err(|_| fmt::Error)?; }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!($crate::print::DebugConsole, $($arg)*);
        }
    };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") }; // Allows us to use println!() to print a newline.
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = writeln!($crate::print::DebugConsole, $($arg)*);
        }
    };
}
