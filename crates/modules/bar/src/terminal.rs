//! Painel "Terminal": os perfis do Windows Terminal (ícone e nome) e, pelo lápis, a edição que
//! esconde e ordena os perfis arrastando, como o "Editar barra".

/// Perfis visíveis (na ordem escolhida) e ocultos (sempre na ordem do Terminal), por índice.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrangement { pub visible: Vec<usize>, pub hidden: Vec<usize> }

/// Onde um perfil arrastado cai: uma posição entre os visíveis (contada sem ele) ou "Ocultos".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot { Visible(usize), Hidden }

impl Arrangement {
    /// Olho: um visível vai para os ocultos; um oculto vai para o fim dos visíveis.
    pub fn toggle(&mut self, profile: usize) {
        if self.visible.contains(&profile) {
            self.move_to(profile, Slot::Hidden);
        } else {
            self.move_to(profile, Slot::Visible(usize::MAX));
        }
    }

    /// Soltou o arraste.
    pub fn move_to(&mut self, profile: usize, slot: Slot) {
        self.visible.retain(|&p| p != profile);
        self.hidden.retain(|&p| p != profile);
        match slot {
            Slot::Visible(index) => self.visible.insert(index.min(self.visible.len()), profile),
            Slot::Hidden => {
                let at = self.hidden.partition_point(|&p| p < profile);
                self.hidden.insert(at, profile);
            }
        }
    }

    /// Ctrl+↑/↓: sobe ou desce entre os visíveis; um oculto não se move.
    pub fn nudge(&mut self, profile: usize, up: bool) {
        let Some(i) = self.visible.iter().position(|&p| p == profile) else { return };
        let j = if up { i.checked_sub(1) } else { (i + 1 < self.visible.len()).then_some(i + 1) };
        if let Some(j) = j {
            self.visible.swap(i, j);
        }
    }

    /// Linhas na ordem da tela (visíveis e depois ocultos), para as setas.
    pub fn rows(&self) -> Vec<usize> {
        self.visible.iter().chain(&self.hidden).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a() -> Arrangement {
        Arrangement { visible: vec![0, 1, 3], hidden: vec![2] }
    }

    #[test]
    fn eye_hides_and_shows() {
        let mut x = a();
        x.toggle(1);
        assert_eq!(x, Arrangement { visible: vec![0, 3], hidden: vec![1, 2] });
        x.toggle(2);
        assert_eq!(x, Arrangement { visible: vec![0, 3, 2], hidden: vec![1] });
    }

    #[test]
    fn drag_reorders_hides_and_shows() {
        let mut x = a();
        x.move_to(3, Slot::Visible(0));
        assert_eq!(x.visible, [3, 0, 1]);
        x.move_to(0, Slot::Hidden);
        assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![3, 1], vec![0, 2]));
        x.move_to(2, Slot::Visible(1));
        assert_eq!((x.visible, x.hidden), (vec![3, 2, 1], vec![0]));
    }

    #[test]
    fn nudge_moves_within_visible_only() {
        let mut x = a();
        x.nudge(3, true);
        assert_eq!(x.visible, [0, 3, 1]);
        x.nudge(0, true);
        x.nudge(2, false);
        assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![0, 3, 1], vec![2]));
    }

    #[test]
    fn hiding_everything_is_allowed() {
        let mut x = a();
        for i in [0, 1, 3] {
            x.toggle(i);
        }
        assert_eq!((x.visible.len(), x.hidden), (0, vec![0, 1, 2, 3]));
    }

    #[test]
    fn rows_list_visible_then_hidden() {
        assert_eq!(a().rows(), [0, 1, 3, 2]);
    }
}
