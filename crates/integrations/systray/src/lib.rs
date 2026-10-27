//! Bandeja do sistema, portável: os bytes que o `Shell_NotifyIcon` manda à `Shell_TrayWnd`
//! (`wire`), a lista de ícones que eles montam (`model`) e as mensagens que devolvem um clique ao
//! app (`click`). A janela e os handles ficam no `platform-windows` (ADR-0023).

pub mod click;
pub mod model;
pub mod wire;
