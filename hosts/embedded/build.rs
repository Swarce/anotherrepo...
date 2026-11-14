fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        // 16 KiB-page compatible (Android 15+ devices may use 16 KiB pages).
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,max-page-size=16384");
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libjdr.so");
    }
}
