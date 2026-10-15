//! Datas e a grade do mês (sem depender do sistema).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date { pub year: i32, pub month: u32, pub day: u32 }

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// Algoritmo de Sakamoto. 0 = domingo.
pub fn weekday(d: Date) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if d.month < 3 { d.year - 1 } else { d.year };
    ((y + y / 4 - y / 100 + y / 400 + T[(d.month - 1) as usize] + d.day as i32).rem_euclid(7)) as u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonthGrid {
    pub year: i32,
    pub month: u32,
    pub first_weekday: u32,
    pub weeks: usize,
    pub cells: Vec<Option<u32>>,
    pub today: Option<u32>,
}

pub fn month_grid(year: i32, month: u32, first_weekday: u32, today: Date) -> MonthGrid {
    let lead = ((weekday(Date { year, month, day: 1 }) + 7 - first_weekday % 7) % 7) as usize;
    let days = days_in_month(year, month) as usize;
    let weeks = (lead + days).div_ceil(7);
    let mut cells = vec![None; weeks * 7];
    for d in 0..days {
        cells[lead + d] = Some(d as u32 + 1);
    }
    let today = (today.year == year && today.month == month).then_some(today.day);
    MonthGrid { year, month, first_weekday, weeks, cells, today }
}

pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let idx = year * 12 + (month as i32 - 1) + delta;
    (idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32)
}

pub fn ms_to_next_minute(second: u32, millis: u32) -> u32 {
    60_000 - (second * 1000 + millis).min(59_999)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_in_month_handles_leap_years() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 10), 31);
        assert_eq!(days_in_month(2026, 11), 30);
    }

    #[test]
    fn weekday_of_known_dates() {
        assert_eq!(weekday(Date { year: 2026, month: 10, day: 15 }), 4); // quinta
        assert_eq!(weekday(Date { year: 2000, month: 1, day: 1 }), 6); // sábado
    }

    #[test]
    fn october_2026_starting_sunday_has_5_weeks() {
        let g = month_grid(2026, 10, 0, Date { year: 2026, month: 10, day: 15 });
        assert_eq!(g.weeks, 5);
        assert_eq!(g.cells.len(), 35);
        assert_eq!(&g.cells[..5], &[None, None, None, None, Some(1)]); // 1º de outubro é quinta
        assert_eq!(g.today, Some(15));
    }

    #[test]
    fn starting_monday_shifts_the_grid() {
        let g = month_grid(2026, 10, 1, Date { year: 2026, month: 9, day: 1 });
        assert_eq!(&g.cells[..4], &[None, None, None, Some(1)]);
        assert_eq!(g.today, None); // hoje está em outro mês
    }

    #[test]
    fn four_and_six_week_months() {
        assert_eq!(month_grid(2026, 2, 0, Date { year: 2026, month: 2, day: 1 }).weeks, 4); // fev/2026 começa no domingo
        assert_eq!(month_grid(2026, 8, 0, Date { year: 2026, month: 8, day: 1 }).weeks, 6); // ago/2026 começa no sábado
    }

    #[test]
    fn shift_month_crosses_years() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 10, -22), (2024, 12));
    }

    #[test]
    fn next_minute() {
        assert_eq!(ms_to_next_minute(0, 0), 60_000);
        assert_eq!(ms_to_next_minute(59, 900), 100);
        assert_eq!(ms_to_next_minute(30, 0), 30_000);
    }
}
