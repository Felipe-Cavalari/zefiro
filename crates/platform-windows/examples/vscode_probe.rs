//! Sonda só de leitura: o Code.exe encontrado e os projetos recentes que o painel mostraria.
fn main() {
    println!("Code.exe: {:?}", zefiro_platform_windows::system::find_code_exe());
    let dir = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Code\User\workspaceStorage");
    for r in zefiro_integration_vscode::read_recents(&dir, &|p| p.exists(), 20) {
        println!("{:?} | {} | {} | {:?}", r.kind, r.name, r.location, zefiro_integration_vscode::open_args(&r));
    }
}
