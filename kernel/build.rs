fn main() {
    // Add rustc linker arguments
    println!("cargo:rustc-link-arg=--Map=kernel/kernel.map");
    println!("cargo:rustc-link-arg=--script=kernel/kernel.ld");

    // Tell cargo to rerun if the linker script changes
    println!("cargo:rerun-if-changed=kernel.ld");
}
