fn main() {
    // Only embed manifest when building the standalone binary, not when used as a library
    #[cfg(target_os = "windows")]
    {
        if std::env::var("CARGO_BIN_NAME").is_ok() {
            let mut res = winres::WindowsResource::new();
            res.set_manifest_file("manifest.xml");
            if let Err(e) = res.compile() {
                eprintln!("Warning: Failed to embed manifest: {}", e);
            }
        }
    }
}
