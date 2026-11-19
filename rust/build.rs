// Icône et métadonnées de l'exécutable Windows.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Annexe PDF");
        res.set("FileDescription", "Annexe PDF");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icône Windows non intégrée : {e}");
        }
    }
    println!("cargo:rerun-if-changed=icon.ico");
}
