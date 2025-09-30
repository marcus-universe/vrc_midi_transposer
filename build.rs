fn main() {
    // Only run winres on Windows targets
    #[cfg(target_os = "windows")]
    {
        // Use winres to embed the icon into the final executable
        let mut res = winres::WindowsResource::new();
        // Path is relative to the crate root
        res.set_icon("src/icon.ico");
    // Optional: set product name and file description using resource string keys
    // winres exposes a generic `set` API for version/resource strings
    res.set("ProductName", "transposer2025");
    res.set("FileDescription", "MIDI Transposer - transposer2025");
        // Compile resources
        match res.compile() {
            Ok(_) => println!("cargo:warning=winres: icon embedded"),
            Err(e) => println!("cargo:warning=winres failed: {}", e),
        }
    }
}
