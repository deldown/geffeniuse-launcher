// Links the app icon in when scripts/build-installer.sh has compiled payload/icon.res.
fn main() {
    let res = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("payload/icon.res");
    println!("cargo:rerun-if-changed={}", res.display());
    if res.exists() && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg={}", res.display());
    }
}
