use std::path::Path;

fn main() {
    let link_script = Path::new(env!("CARGO_MANIFEST_DIR")).join("linker.ld");

    println!(
        "cargo:rustc-link-arg-bins=--script={}",
        link_script.display()
    );
}
