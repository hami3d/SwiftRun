fn main() {
    // Only run on Windows
    #[cfg(windows)]
    {
        // Tell Cargo to link the manifest
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:app.manifest");

        // Compile resources
        let _ = embed_resource::compile("resources.rc", embed_resource::NONE);
    }
}
