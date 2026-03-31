fn main() {
    // Windows: embed icon into EXE
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Kolibri Ai3D");
        res.set("FileDescription", "Kolibri Ai3D - 3D/2D CAD Modeling");
        res.set("CompanyName", "Kolibri");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to set icon: {}", e);
        }
    }
}
