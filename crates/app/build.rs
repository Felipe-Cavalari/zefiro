//! Ícone e dados de versão do `zefiro.exe` (Explorer, Gerenciador de Tarefas, instalador).

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icons/app.ico")
        .set("ProductName", "Zefiro")
        .set("FileDescription", "Zefiro — barra e ilha de mídia para o Windows")
        .set("CompanyName", "Felipe Cavalari")
        .set("LegalCopyright", "MIT OR Apache-2.0")
        .set("OriginalFilename", "zefiro.exe");
    if let Err(e) = res.compile() {
        // Sem o compilador de recursos (rc.exe do Windows SDK), o exe sai sem ícone, mas sai.
        println!("cargo:warning=ícone e versão não embutidos: {e}");
    }
}
