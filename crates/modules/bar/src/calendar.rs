//! Calendário do mês, no estilo do calendário da ilha.

use zefiro_integration_clock::format::ClockText;
use zefiro_integration_clock::grid::{Date, MonthGrid};
use zefiro_platform::geometry::{Point, Rect, Size};
use zefiro_platform::text::{Align, Family, FontWeight, TextStyle};
use zefiro_resources::tokens::Tokens;
use zefiro_ui::paint::{DisplayList, DrawOp};

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarNames { pub month: String, pub year: String, pub weekday_long: String, pub today_day: String, pub initials: [String; 7] }

pub fn calendar_names(text: &dyn ClockText, shown_year: i32, shown_month: u32, today: Date) -> CalendarNames {
    CalendarNames {
        month: text.month_name(shown_year, shown_month),
        year: shown_year.to_string(),
        weekday_long: text.weekday_long(today),
        today_day: today.day.to_string(),
        initials: text.weekday_initials(),
    }
}

/// Tamanho fixo, o da ilha (largura sem a coluna da paginação): a grade divide a altura entre as semanas.
pub fn calendar_size(_weeks: usize, scale: f32) -> Size {
    Size { w: 352.0 * scale, h: 184.0 * scale }
}

/// Mesmo desenho do `DrawCalendarDashboard` da ilha: herói com mês (destaque), ano, o dia em
/// 63 px e o dia da semana; grade com iniciais discretas, linha fina, fim de semana recuado e
/// hoje como o único destaque.
pub fn calendar_paint(grid: &MonthGrid, names: &CalendarNames, scale: f32, tokens: &Tokens) -> DisplayList {
    let s = |v: f32| v * scale;
    let size = calendar_size(grid.weeks, scale);
    let mut l = DisplayList::new(size);
    let st = |px: f32, weight: FontWeight, family: Family| TextStyle { size: s(px), weight, family };

    // Herói.
    let hero = Rect::new(s(22.0), s(16.0), s(93.0), size.h - s(16.0) - s(20.0));
    l.push(DrawOp::RoundRect { rect: hero, radius: s(12.0), color: tokens.raised });
    l.push(DrawOp::Text { rect: Rect::new(hero.x + s(3.0), hero.y + s(6.0), hero.w - s(6.0), s(16.0)), text: names.month.clone(), style: st(12.0, FontWeight::Bold, Family::Text), color: tokens.accent, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x, hero.y + s(21.0), hero.w, s(15.0)), text: names.year.clone(), style: st(11.0, FontWeight::Regular, Family::Text), color: tokens.text_secondary, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x, hero.y + s(34.0), hero.w, hero.h - s(60.0)), text: names.today_day.clone(), style: st(63.0, FontWeight::Bold, Family::Display), color: tokens.text, align: Align::Center });
    l.push(DrawOp::Text { rect: Rect::new(hero.x + s(4.0), hero.bottom() - s(24.0), hero.w - s(8.0), s(18.0)), text: names.weekday_long.clone(), style: st(12.0, FontWeight::Bold, Family::Text), color: tokens.text_secondary, align: Align::Center });

    // Grade.
    let (x0, y0, col, header) = (s(134.0), s(16.0), s(28.0), s(18.0));
    let dates_top = y0 + header + s(7.0);
    let row = (size.h - s(14.0) - dates_top) / grid.weeks.max(1) as f32;
    let grid_style = st(14.4, FontWeight::Bold, Family::Text);
    for c in 0..7 {
        let weekday = (grid.first_weekday as usize + c) % 7;
        l.push(DrawOp::Text { rect: Rect::new(x0 + col * c as f32, y0, col, header), text: names.initials[weekday].clone(), style: grid_style, color: tokens.text_tertiary, align: Align::Center });
    }
    l.push(DrawOp::RoundRect { rect: Rect::new(x0 + s(2.0), y0 + header + s(3.0), col * 7.0 - s(4.0), s(1.0)), radius: 0.0, color: tokens.hairline });
    let marker = col.min(row) * 0.5 - s(1.5);
    for (i, cell) in grid.cells.iter().enumerate() {
        let Some(day) = cell else { continue };
        let rect = Rect::new(x0 + col * (i % 7) as f32, dates_top + row * (i / 7) as f32, col, row);
        let weekday = (grid.first_weekday as usize + i % 7) % 7;
        let color = if grid.today == Some(*day) {
            l.push(DrawOp::Circle { center: Point { x: rect.center_x(), y: rect.y + row / 2.0 }, radius: marker, color: tokens.accent });
            tokens.text
        } else if weekday == 0 || weekday == 6 {
            tokens.text_secondary
        } else {
            tokens.text
        };
        l.push(DrawOp::Text { rect, text: day.to_string(), style: grid_style, color, align: Align::Center });
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_clock::format::PtBrFallback;
    use zefiro_integration_clock::grid::{Date, month_grid};
    use zefiro_platform::color::Rgba;
    use zefiro_resources::theme::{BUILTIN, parse_theme};
    use zefiro_resources::tokens::derive_tokens;
    use zefiro_ui::paint::DrawOp;

    const TODAY: Date = Date { year: 2026, month: 10, day: 15 };

    fn tokens() -> zefiro_resources::tokens::Tokens {
        derive_tokens(&parse_theme("obsidian", "o", BUILTIN[0].1).unwrap(), Rgba::rgb(0, 120, 215), None)
    }

    #[test]
    fn size_is_the_islands_and_does_not_depend_on_weeks() {
        // Como na ilha (184 de altura): a grade divide a altura disponível entre as semanas.
        assert_eq!(calendar_size(5, 1.0), Size { w: 352.0, h: 184.0 });
        assert_eq!(calendar_size(6, 1.0), Size { w: 352.0, h: 184.0 });
        assert_eq!(calendar_size(4, 2.0), Size { w: 704.0, h: 368.0 });
    }

    fn day_op<'a>(list: &'a DisplayList, day: &str) -> &'a DrawOp {
        // O primeiro texto "15" é o número grande do herói; os da grade vêm depois dele.
        list.ops.iter().rfind(|o| matches!(o, DrawOp::Text { text, .. } if text == day)).unwrap()
    }

    #[test]
    fn looks_like_the_islands_calendar() {
        let g = month_grid(2026, 10, 0, TODAY);
        let t = tokens();
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 10, TODAY), 1.0, &t);
        // Fim de semana recua (sábado 3, domingo 4); dia útil em texto normal.
        let color = |day: &str| match day_op(&list, day) { DrawOp::Text { color, .. } => *color, _ => unreachable!() };
        assert_eq!(color("3"), t.text_secondary);
        assert_eq!(color("4"), t.text_secondary);
        assert_eq!(color("5"), t.text);
        // Hoje: texto claro em cima do círculo de destaque.
        assert_eq!(color("15"), t.text);
        // Número grande do herói no tamanho da ilha (63, negrito).
        let hero = list.ops.iter().find(|o| matches!(o, DrawOp::Text { text, .. } if text == "15")).unwrap();
        assert!(matches!(hero, DrawOp::Text { style, .. } if style.size == 63.0 && style.weight == FontWeight::Bold), "{hero:?}");
        // Linha fina embaixo das iniciais.
        assert!(list.ops.iter().any(|o| matches!(o, DrawOp::RoundRect { rect, color, .. } if rect.h == 1.0 && *color == t.hairline)));
    }

    #[test]
    fn six_week_month_fits_inside_the_panel() {
        let g = month_grid(2026, 8, 0, TODAY); // agosto de 2026: 6 semanas
        assert_eq!(g.weeks, 6);
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 8, TODAY), 1.0, &tokens());
        let DrawOp::Text { rect, .. } = day_op(&list, "31") else { unreachable!() };
        assert!(rect.bottom() <= list.size.h - 14.0 + 0.01, "{rect:?} em {:?}", list.size);
    }

    #[test]
    fn names_use_shown_month_but_today_for_the_hero_day() {
        let n = calendar_names(&PtBrFallback, 2026, 11, TODAY);
        assert_eq!(n.month, "novembro");
        assert_eq!(n.year, "2026");
        assert_eq!(n.today_day, "15");
        assert_eq!(n.weekday_long, "quinta-feira");
    }

    #[test]
    fn today_gets_an_accent_circle_and_every_day_is_drawn() {
        let g = month_grid(2026, 10, 0, TODAY);
        let t = tokens();
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 10, TODAY), 1.0, &t);
        let circles = list.ops.iter().filter(|o| matches!(o, DrawOp::Circle { color, .. } if *color == t.accent)).count();
        assert_eq!(circles, 1);
        let numbers = list.ops.iter().filter(|o| matches!(o, DrawOp::Text { text, .. } if text.parse::<u32>().is_ok_and(|d| d <= 31))).count();
        assert_eq!(numbers, 31 + 1); // 31 dias + o número grande do herói
        assert_eq!(list.size, calendar_size(5, 1.0));
    }

    #[test]
    fn other_month_has_no_today_circle() {
        let g = month_grid(2026, 11, 0, TODAY);
        let t = tokens();
        let list = calendar_paint(&g, &calendar_names(&PtBrFallback, 2026, 11, TODAY), 1.0, &t);
        assert!(!list.ops.iter().any(|o| matches!(o, DrawOp::Circle { color, .. } if *color == t.accent)));
    }
}
