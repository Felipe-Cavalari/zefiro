//! Ícones de marca dos chips, em SVG monocromático (`currentColor`), da coleção do devicons.io
//! (versões "font"): Claude Code (Anthropic), Codex (OpenAI, só a nuvem com `>_`, sem o nome ao lado)
//! e Visual Studio Code (Microsoft); e o ícone do Spotify (Simple Icons, CC0), desenhado no verde da
//! marca. As marcas pertencem às empresas; ficam aqui só para identificar o serviço de cada chip.

pub const CLAUDE_CODE: &str = include_str!("../icons/claude-code.svg");
pub const CODEX: &str = include_str!("../icons/codex.svg");
pub const VSCODE: &str = include_str!("../icons/visual-studio-code.svg");
pub const SPOTIFY: &str = include_str!("../icons/spotify.svg");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_is_only_the_cloud_without_the_wordmark() {
        assert_eq!(CODEX.matches("<path").count(), 1, "só o ícone");
        assert!(CODEX.contains(r#"viewBox="-2 226 148 148""#), "quadrado em volta da nuvem");
    }

    #[test]
    fn icons_are_tintable_and_scale_with_their_view_box() {
        for svg in [CLAUDE_CODE, CODEX, VSCODE, SPOTIFY] {
            assert!(svg.starts_with("<svg "), "o painter injeta a opacidade logo depois de `<svg `");
            assert!(svg.contains("currentColor"), "a cor vem do tema");
            assert!(svg.contains("viewBox="));
            assert!(!svg.contains(" width=\"") && !svg.contains(" height=\""), "sem tamanho fixo: o retângulo do chip manda");
        }
    }
}
