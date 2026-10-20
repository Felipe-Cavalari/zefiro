//! Layout da barra nas 2 alturas × 3 formatos (spec §5.1; ilhas no ADR-0016).

use crate::panel_layout::Shape;
use zefiro_core::config::{BarFormat, BarHeight};
use zefiro_platform::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarMetrics {
    pub band: f32,
    pub pill: f32,
    pub chip_h: f32,
    pub island_pill: f32,
    pub group_pad: f32,
    pub edge: f32,
    pub chip_pad_x: f32,
    pub chip_gap: f32,
    pub font: f32,
    /// Ilhas: altura do trilho no topo, canto de baixo das ilhas e concordância com o trilho. 0 nos outros formatos.
    pub rail: f32,
    pub island_radius: f32,
    pub fillet: f32,
}

pub fn metrics(height: BarHeight, format: BarFormat) -> BarMetrics {
    let (band, pill, chip_h, island_pill, group_pad, font) = match (height, format) {
        (BarHeight::Completa, BarFormat::Faixa) => (36.0, 28.0, 28.0, 30.0, 0.0, 13.0),
        (BarHeight::Compacta, BarFormat::Faixa) => (32.0, 24.0, 24.0, 28.0, 0.0, 12.0),
        (BarHeight::Completa, BarFormat::Pilulas) => (36.0, 30.0, 26.0, 30.0, 2.0, 13.0),
        (BarHeight::Compacta, BarFormat::Pilulas) => (32.0, 24.0, 20.0, 24.0, 2.0, 12.0),
        // A ilha tem a altura da barra; números do YASB: railheight 4, islandradius 16, edgeradius 8, grouppadding 12.
        (BarHeight::Completa, BarFormat::Ilhas) => (36.0, 36.0, 28.0, 28.0, 12.0, 13.0),
        (BarHeight::Compacta, BarFormat::Ilhas) => (32.0, 32.0, 24.0, 24.0, 12.0, 12.0),
    };
    let base = BarMetrics { band, pill, chip_h, island_pill, group_pad, edge: 8.0, chip_pad_x: 10.0, chip_gap: 4.0, font, rail: 0.0, island_radius: 0.0, fillet: 0.0 };
    match format {
        // As ilhas das pontas encostam na borda do monitor.
        BarFormat::Ilhas => BarMetrics { edge: 0.0, rail: 4.0, island_radius: 16.0, fillet: 8.0, ..base },
        _ => base,
    }
}

/// Lado da ilha em que fica a concordância (o canto côncavo entre o trilho e a ilha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilletSide { Left, Right }

/// Quadrado da concordância, logo abaixo do trilho e encostado na ilha. O preenchido é o canto junto
/// dos dois (em cima, do lado da ilha); o resto é um quarto de círculo vazio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fillet { pub rect: Rect, pub side: FilletSide }

/// Formato ilhas, em px. O trilho só existe nos vãos entre as ilhas: nada se sobrepõe, então o
/// acrílico e a opacidade do tema não fazem emenda mais escura.
#[derive(Debug, Clone, PartialEq)]
pub struct Islands { pub rail_h: f32, pub radius: f32, pub fillet: f32, pub rails: Vec<Rect>, pub fillets: Vec<Fillet> }

/// `wide`: o chip não tem o teto de 160 DIP (o player, que tem a largura da pílula da ilha).
#[derive(Debug, Clone, PartialEq)]
pub struct ChipSpec { pub id: String, pub content_w: f32, pub wide: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct ChipBox { pub id: String, pub rect: Rect }

#[derive(Debug, Clone, PartialEq)]
pub struct OverflowGroup { pub id: String, pub ids: Vec<String> }

#[derive(Debug, Clone, PartialEq)]
pub struct BarLayout {
    pub scale: f32,
    pub band: Rect,
    pub strip: Option<Rect>,
    pub groups: Vec<Rect>,
    pub chips: Vec<ChipBox>,
    pub island_pill_h: f32,
    pub font_px: f32,
    pub chip_h_px: f32,
    pub overflow: Vec<OverflowGroup>,
    pub regions: [Rect; 3],
    /// Só no formato ilhas; aí `groups` são as ilhas visíveis (do topo ao pé da barra).
    pub islands: Option<Islands>,
}

impl BarLayout {
    /// Forma de um grupo. Nas ilhas, o retângulo sobe um raio acima da tela (os cantos de cima somem) e
    /// passa um raio da borda do monitor quando encosta nela (o canto de fora fica reto).
    pub fn group_shape(&self, r: Rect, pill_radius: f32) -> Shape {
        let Some(islands) = &self.islands else { return Shape { rect: r, radius: pill_radius } };
        let radius = islands.radius;
        let left = if r.x <= 0.0 { radius } else { 0.0 };
        let right = if r.right() >= self.band.w { radius } else { 0.0 };
        Shape { rect: Rect::new(r.x - left, r.y - radius, r.w + left + right, r.h + radius), radius }
    }

    /// Topo da pílula da ilha de mídia: centrada na barra, ou abaixo do trilho nas ilhas.
    pub fn island_pill_y(&self) -> f32 {
        let top = self.islands.as_ref().map_or(0.0, |i| i.rail_h);
        (top + (self.band.h - top - self.island_pill_h) / 2.0).round()
    }

    /// Seção do grupo (0 início, 1 centro, 2 fim): a região que contém o meio dele, ou a mais próxima.
    /// Assim cada pílula ou ilha nasce e some no próprio lugar quando outra seção aparece ou some.
    pub fn group_slot(&self, r: Rect) -> usize {
        let x = r.center_x();
        let distance = |reg: &Rect| if x < reg.x { reg.x - x } else if x > reg.right() { x - reg.right() } else { 0.0 };
        (0..3).min_by(|&a, &b| distance(&self.regions[a]).total_cmp(&distance(&self.regions[b]))).unwrap_or(0)
    }

    /// A ilha de mídia se funde à barra (sem contorno nem sombra): na faixa e nas ilhas.
    pub fn merges_island(&self) -> bool {
        self.strip.is_some() || self.islands.is_some()
    }
}

/// Trilho nos vãos entre as ilhas e concordâncias nos lados que não encostam na borda. Um vão estreito
/// demais para as duas concordâncias fica sem elas.
fn island_parts(groups: &[Rect], monitor_w: f32, rail: f32, fillet: f32) -> (Vec<Rect>, Vec<Fillet>) {
    let mut sorted = groups.to_vec();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
    let (mut rails, mut fillets) = (Vec::new(), Vec::new());
    let room = |gap: f32, shared: bool| gap >= if shared { 2.0 * fillet } else { fillet };
    let mut cursor = 0.0f32;
    for (i, g) in sorted.iter().enumerate() {
        if g.x > cursor {
            rails.push(Rect::new(cursor, 0.0, g.x - cursor, rail));
        }
        if g.x > 0.0 && room(g.x - cursor, i > 0) {
            fillets.push(Fillet { rect: Rect::new(g.x - fillet, rail, fillet, fillet), side: FilletSide::Left });
        }
        let next = sorted.get(i + 1).map_or(monitor_w, |n| n.x);
        if g.right() < monitor_w && room(next - g.right(), i + 1 < sorted.len()) {
            fillets.push(Fillet { rect: Rect::new(g.right(), rail, fillet, fillet), side: FilletSide::Right });
        }
        cursor = cursor.max(g.right());
    }
    if cursor < monitor_w {
        rails.push(Rect::new(cursor, 0.0, monitor_w - cursor, rail));
    }
    (rails, fillets)
}

fn finish_islands(layout: &mut BarLayout, m: &BarMetrics, monitor_w: f32, scale: f32) {
    let px = |dip: f32| (dip * scale).round();
    let (rail, fillet) = (px(m.rail), px(m.fillet));
    let (rails, fillets) = island_parts(&layout.groups, monitor_w, rail, fillet);
    layout.islands = Some(Islands { rail_h: rail, radius: px(m.island_radius), fillet, rails, fillets });
}

fn rounded(r: Rect) -> Rect {
    Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round())
}

/// `content_w` de cada chip já está em px. Início alinhado à esquerda, fim à direita.
pub fn layout_bar(monitor_w: f32, scale: f32, height: BarHeight, format: BarFormat, start: &[ChipSpec], end: &[ChipSpec]) -> BarLayout {
    let m = metrics(height, format);
    let px = |dip: f32| (dip * scale).round();
    let band = Rect::new(0.0, 0.0, monitor_w, px(m.band));
    let chip_h = px(m.chip_h);
    let pad = px(m.group_pad);
    let gap = px(m.chip_gap);
    let chip_w = |c: &ChipSpec| (c.content_w + 2.0 * px(m.chip_pad_x)).round();
    let section_w = |cs: &[ChipSpec]| cs.iter().map(chip_w).sum::<f32>() + gap * cs.len().saturating_sub(1) as f32;

    let mut groups = Vec::new();
    let mut chips = Vec::new();
    let pills = format == BarFormat::Pilulas;
    let group_h = px(m.pill);
    let group_y = ((band.h - group_h) / 2.0).round();
    let chip_y = if pills { group_y + pad } else { ((band.h - chip_h) / 2.0).round() };

    let mut place = |cs: &[ChipSpec], left: f32| {
        if cs.is_empty() {
            return;
        }
        if pills {
            groups.push(rounded(Rect::new(left, group_y, section_w(cs) + 2.0 * pad, group_h)));
        }
        let mut x = left + if pills { pad } else { 0.0 };
        for c in cs {
            let w = chip_w(c);
            chips.push(ChipBox { id: c.id.clone(), rect: rounded(Rect::new(x, chip_y, w, chip_h)) });
            x += w + gap;
        }
    };
    let edge = px(m.edge);
    place(start, edge);
    let end_w = section_w(end) + if pills { 2.0 * pad } else { 0.0 };
    place(end, monitor_w - edge - end_w);

    BarLayout {
        scale,
        band,
        strip: (format == BarFormat::Faixa).then_some(band),
        groups,
        chips,
        island_pill_h: px(m.island_pill),
        font_px: m.font * scale,
        chip_h_px: chip_h,
        overflow: Vec::new(),
        regions: [Rect::default(); 3],
        islands: None,
    }
}

/// Layout das três seções; reserva da ilha e medições já estão em pixels físicos.
pub fn layout_sections(monitor_w: f32, scale: f32, height: BarHeight, format: BarFormat, sections: [&[ChipSpec]; 3], island_width: Option<f32>) -> BarLayout {
    let mut layout = layout_bar(monitor_w, scale, height, format, &[], &[]);
    let m = metrics(height, format);
    let edge = (m.edge * scale).round().min(monitor_w.max(0.0) / 2.0);
    let available = (monitor_w - 2.0 * edge).max(0.0);
    let gap = (m.chip_gap * scale).round();
    let islands = format == BarFormat::Ilhas;
    let pad = if format == BarFormat::Faixa { 0.0 } else { (m.group_pad * scale).round() };
    let top = (m.rail * scale).round();
    let chip_y = (top + (layout.band.h - top - layout.chip_h_px) / 2.0).round();
    let width = |chip: &ChipSpec| (chip.content_w.min(if chip.wide { f32::MAX } else { 160.0 * scale }).max(0.0) + 2.0 * (m.chip_pad_x * scale).round()).ceil();
    let total = |items: &[ChipSpec]| items.iter().map(&width).sum::<f32>() + gap * items.len().saturating_sub(1) as f32 + if items.is_empty() { 0.0 } else { 2.0 * pad };
    // Nas ilhas, a ilha do centro envolve a reserva da ilha de mídia com a margem dos dois lados.
    let island_width = island_width.map(|w| if islands && w > 0.0 { w + 2.0 * pad } else { w });
    let center = island_width.unwrap_or_else(|| total(sections[1]).min(available / 2.0)).max(0.0).min(available);
    let separation = if center > 0.0 { (8.0 * scale).round() } else { 0.0 };
    let side = ((available - center - 2.0 * separation) / 2.0).max(0.0);
    let more_width = (28.0 * scale).round();
    layout.regions = [
        Rect::new(edge, 0.0, side, layout.band.h),
        Rect::new((monitor_w - center) / 2.0, 0.0, center, layout.band.h),
        Rect::new(monitor_w - edge - side, 0.0, side, layout.band.h),
    ];
    let needs_recovery = sections.iter().enumerate().any(|(i, items)| {
        !(items.is_empty() || (i == 1 && island_width.is_some())) && total(items) > layout.regions[i].w && layout.regions[i].w < more_width + 2.0 * pad
    });
    if needs_recovery {
        let ids = sections.iter().enumerate().filter(|(i, _)| *i != 1 || island_width.is_none()).flat_map(|(_, items)| items.iter().map(|c| c.id.clone())).collect();
        layout.overflow.push(OverflowGroup { id: "__more_all".into(), ids });
        if available > 0.0 {
            let (x, w) = if islands { (edge + pad, (available - 2.0 * pad).max(1.0).min(more_width)) } else { (edge, available.min(more_width)) };
            layout.chips.push(ChipBox { id: "__more_all".into(), rect: Rect::new(x, chip_y, w, layout.chip_h_px) });
            if islands {
                layout.groups.push(Rect::new(edge, 0.0, w + 2.0 * pad, layout.band.h));
                if center > 0.0 && island_width.is_some() {
                    layout.groups.push(rounded(Rect::new(layout.regions[1].x, 0.0, center, layout.band.h)));
                }
            }
        }
        if islands {
            finish_islands(&mut layout, &m, monitor_w, scale);
        }
        return layout;
    }
    for (index, items) in sections.into_iter().enumerate() {
        if index == 1 && island_width.is_some() {
            if islands && center > 0.0 {
                layout.groups.push(rounded(Rect::new(layout.regions[1].x, 0.0, center, layout.band.h)));
            }
            continue;
        }
        if items.is_empty() { continue; }
        let region = layout.regions[index];
        let mut count = items.len();
        let mut used = total(items);
        while count > 0 && used > region.w {
            count -= 1;
            used = total(&items[..count]) + more_width + if count > 0 { gap } else { 2.0 * pad };
        }
        let has_more = count < items.len();
        let x = match index { 0 => region.x, 1 => region.x + (region.w - used) / 2.0, _ => region.right() - used };
        match format {
            BarFormat::Pilulas => layout.groups.push(rounded(Rect::new(x, (layout.band.h - m.pill * scale) / 2.0, used, m.pill * scale))),
            BarFormat::Ilhas => layout.groups.push(rounded(Rect::new(x, 0.0, used, layout.band.h))),
            BarFormat::Faixa => {}
        }
        let mut cursor = x + pad;
        let y = chip_y;
        for item in &items[..count] {
            let w = width(item);
            layout.chips.push(ChipBox { id: item.id.clone(), rect: rounded(Rect::new(cursor, y, w, layout.chip_h_px)) });
            cursor += w + gap;
        }
        if has_more {
            let id = format!("__more_{index}");
            layout.chips.push(ChipBox { id: id.clone(), rect: rounded(Rect::new(cursor, y, more_width, layout.chip_h_px)) });
            layout.overflow.push(OverflowGroup { id, ids: items[count..].iter().map(|item| item.id.clone()).collect() });
        }
    }
    if islands {
        finish_islands(&mut layout, &m, monitor_w, scale);
    }
    layout
}

pub fn chip_at(layout: &BarLayout, p: Point) -> Option<&ChipBox> {
    layout.chips.iter().find(|c| c.rect.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_core::config::{BarFormat, BarHeight};
    use zefiro_platform::geometry::{Point, Rect};

    fn chips(ids: &[(&str, f32)]) -> Vec<ChipSpec> {
        ids.iter().map(|(id, w)| ChipSpec { id: id.to_string(), content_w: *w, wide: false }).collect()
    }

    #[test]
    fn metrics_table_matches_the_spec() {
        let m = metrics(BarHeight::Completa, BarFormat::Faixa);
        assert_eq!((m.band, m.chip_h, m.island_pill, m.font), (36.0, 28.0, 30.0, 13.0));
        let m = metrics(BarHeight::Compacta, BarFormat::Faixa);
        assert_eq!((m.band, m.chip_h, m.island_pill, m.font), (32.0, 24.0, 28.0, 12.0));
        let m = metrics(BarHeight::Completa, BarFormat::Pilulas);
        assert_eq!((m.band, m.pill, m.chip_h, m.island_pill, m.group_pad), (36.0, 30.0, 26.0, 30.0, 2.0));
        let m = metrics(BarHeight::Compacta, BarFormat::Pilulas);
        assert_eq!((m.band, m.pill, m.chip_h, m.island_pill, m.group_pad), (32.0, 24.0, 20.0, 24.0, 2.0));
    }

    #[test]
    fn island_pill_never_reaches_the_band_edges() {
        // Do tamanho da faixa, a pílula (com contorno e sombra) parece maior que a barra.
        for h in [BarHeight::Completa, BarHeight::Compacta] {
            for f in [BarFormat::Faixa, BarFormat::Pilulas, BarFormat::Ilhas] {
                let m = metrics(h, f);
                assert!(m.island_pill <= m.band - m.rail - 4.0, "{h:?}/{f:?}: ilha {} na faixa {}", m.island_pill, m.band);
            }
        }
    }

    #[test]
    fn strip_full_height_has_strip_and_no_groups() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Faixa, &chips(&[("zefiro", 60.0)]), &chips(&[("clock", 80.0)]));
        assert_eq!(l.band, Rect::new(0.0, 0.0, 1920.0, 36.0));
        assert_eq!(l.strip, Some(l.band));
        assert!(l.groups.is_empty());
        assert_eq!(l.chips[0].rect, Rect::new(8.0, 4.0, 80.0, 28.0)); // 60 + 2×10
        assert_eq!(l.chips[1].rect, Rect::new(1920.0 - 8.0 - 100.0, 4.0, 100.0, 28.0));
        assert_eq!(l.island_pill_h, 30.0);
    }

    #[test]
    fn pills_wrap_each_section_in_a_group() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Pilulas, &chips(&[("zefiro", 60.0)]), &chips(&[("a", 20.0), ("b", 30.0)]));
        assert_eq!(l.strip, None);
        assert_eq!(l.groups.len(), 2);
        assert_eq!(l.groups[0], Rect::new(8.0, 3.0, 84.0, 30.0)); // 80 + 2×2
        assert_eq!(l.chips[0].rect, Rect::new(10.0, 5.0, 80.0, 26.0));
        // fim: 40 + 4 + 50 + 2×2 = 98, encostado a 8 da direita
        assert_eq!(l.groups[1], Rect::new(1814.0, 3.0, 98.0, 30.0));
        assert_eq!(l.chips[1].rect, Rect::new(1816.0, 5.0, 40.0, 26.0));
        assert_eq!(l.chips[2].rect, Rect::new(1860.0, 5.0, 50.0, 26.0));
    }

    #[test]
    fn empty_section_has_no_group() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Compacta, BarFormat::Pilulas, &[], &chips(&[("clock", 80.0)]));
        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.chips.len(), 1);
    }

    #[test]
    fn scales_and_rounds_at_125_and_150_percent() {
        for scale in [1.25f32, 1.5] {
            let l = layout_bar(2400.0, scale, BarHeight::Compacta, BarFormat::Faixa, &chips(&[("zefiro", 31.0)]), &[]);
            assert_eq!(l.band.h, (32.0 * scale).round());
            let r = l.chips[0].rect;
            for v in [r.x, r.y, r.w, r.h] {
                assert_eq!(v, v.round(), "{scale}: {r:?}");
            }
            assert_eq!(l.font_px, 12.0 * scale);
            assert!(r.bottom() <= l.band.bottom() && r.y >= 0.0);
        }
    }

    #[test]
    fn hit_test_finds_chip() {
        let l = layout_bar(1920.0, 1.0, BarHeight::Completa, BarFormat::Faixa, &chips(&[("zefiro", 60.0)]), &[]);
        assert_eq!(chip_at(&l, Point { x: 10.0, y: 20.0 }).map(|c| c.id.as_str()), Some("zefiro"));
        assert!(chip_at(&l, Point { x: 500.0, y: 20.0 }).is_none());
    }
}
