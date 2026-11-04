//! Sonda só de leitura: o executável encontrado (VS Code, ou Cursor com o argumento `cursor`), o ícone
//! dele e os projetos recentes que o painel mostraria.
use zefiro_integration_vscode::Editor;

fn main() {
    let editor = if std::env::args().nth(1).is_some_and(|a| a.eq_ignore_ascii_case("cursor")) { Editor::Cursor } else { Editor::Code };
    let exe = zefiro_platform_windows::system::find_editor_exe(editor);
    println!("{}: {exe:?}", editor.exe());
    if let Some(icon) = exe.as_deref().and_then(|p| zefiro_platform_windows::app_icon::file_icon(p, 64)) {
        println!("ícone: {}x{}", icon.width(), icon.height());
    }
    let dir = editor.storage_dir(&std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()));
    for r in zefiro_integration_vscode::read_recents(&dir, &|p| p.exists(), 20) {
        println!("{:?} | {} | {} | {:?}", r.kind, r.name, r.location, zefiro_integration_vscode::open_args(&r));
    }
}
