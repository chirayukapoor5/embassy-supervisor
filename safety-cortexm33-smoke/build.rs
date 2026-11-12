use std::{env, fs, path::PathBuf};
fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out.join("memory.x"), include_bytes!("memory.x")).expect("memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg-bin=safety-cortexm33-smoke=-Tlink.x");
    println!("cargo:rerun-if-changed=memory.x");
}
