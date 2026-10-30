//! Cola de comandos do chip `commands` (ADR-0029): o pacote embutido em português, os arquivos do
//! usuário em `%APPDATA%\zefiro\commands` e a busca. Portável: não toca no Windows.

mod model;
mod search;

pub use search::{Hit, fold, terms};
pub use model::{Command, Library, Tool, builtin, merge, parse_tool, read_user_dir};
