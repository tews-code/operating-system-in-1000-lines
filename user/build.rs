fn main() {
    println!("cargo:rustc-link-arg=--Map=user/user.map");
    println!("cargo:rustc-link-arg=--script=user/user.ld");
}
