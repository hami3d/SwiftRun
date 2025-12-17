fn main() {
    // Only run on Windows
    #[cfg(windows)]
    {
        // Tell Cargo to link the manifest
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:app.manifest");
    }
}