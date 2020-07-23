use cc;

use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "macos" {
        cc::Build::new().file("runas-darwin.c").compile("runas");
        println!("cargo:rustc-link-lib=framework=Security");
    }
}
