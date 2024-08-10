fn main() {
    println!("cargo:rustc-env=FOO={}", 100);
}
