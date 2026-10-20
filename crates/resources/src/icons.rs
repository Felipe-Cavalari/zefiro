//! Ícones de marca dos chips, em SVG monocromático (`currentColor`), da coleção do devicons.io
//! (versões "font"): Claude Code (Anthropic), Codex (OpenAI) e Visual Studio Code (Microsoft). As
//! marcas pertencem às empresas; ficam aqui só para identificar o serviço de cada chip.

pub const CLAUDE_CODE: &str = include_str!("../icons/claude-code.svg");
pub const CODEX: &str = include_str!("../icons/codex.svg");
pub const VSCODE: &str = include_str!("../icons/visual-studio-code.svg");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_tintable_and_scale_with_their_view_box() {
        for svg in [CLAUDE_CODE, CODEX, VSCODE] {
            assert!(svg.starts_with("<svg "), "o painter injeta a opacidade logo depois de `<svg `");
            assert!(svg.contains("currentColor"), "a cor vem do tema");
            assert!(svg.contains(r#"viewBox="0 0 600 600""#));
            assert!(!svg.contains(" width=\"") && !svg.contains(" height=\""), "sem tamanho fixo: o retângulo do chip manda");
        }
    }
}
