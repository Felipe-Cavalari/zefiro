//! Textos de uso: quanto falta para reiniciar, a data absoluta em pt-BR e a faixa de alerta.

const WEEKDAYS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const MONTHS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];

/// "2 h 14 min", "14 min", "3 d 4 h" ou "menos de 1 min".
pub fn format_relative(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    if minutes < 1 {
        return "menos de 1 min".into();
    }
    let (days, hours, mins) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    match (days, hours, mins) {
        (0, 0, m) => format!("{m} min"),
        (0, h, 0) => format!("{h} h"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, 0, _) => format!("{d} d"),
        (d, h, _) => format!("{d} d {h} h"),
    }
}

/// Mês e dia de um número de dias desde 1970-01-01 (algoritmo `civil_from_days` de Howard Hinnant).
fn month_day(days: i64) -> (u32, u32) {
    let z = days + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

/// "qui 22 out, 11:08": `unix` (UTC) no fuso `utc_offset_s`.
pub fn format_absolute(unix: i64, utc_offset_s: i32) -> String {
    let local = unix + i64::from(utc_offset_s);
    let (days, secs) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (month, day) = month_day(days);
    // 1970-01-01 foi uma quinta-feira.
    let weekday = (days + 4).rem_euclid(7) as usize;
    format!("{} {day} {}, {:02}:{:02}", WEEKDAYS[weekday], MONTHS[month as usize - 1], secs / 3600, secs % 3600 / 60)
}

/// A partir de 80 % o chip e a barra do painel ficam na cor de destaque.
pub fn is_high(percent: f32) -> bool {
    percent >= 80.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_and_absolute_formats() {
        assert_eq!(format_relative(8040), "2 h 14 min");
        assert_eq!(format_relative(840), "14 min");
        assert_eq!(format_relative(30), "menos de 1 min");
        assert_eq!(format_relative(7200), "2 h");
        assert_eq!(format_relative(3 * 86400 + 4 * 3600 + 120), "3 d 4 h");
        // Conferido com Python: 1792678081 em UTC−3 é quinta, 22/10/2026, 11:08.
        assert_eq!(format_absolute(1792678081, -10800), "qui 22 out, 11:08");
        assert_eq!(format_absolute(0, 0), "qui 1 jan, 00:00");
    }

    #[test]
    fn high_band_starts_at_80() {
        assert!(!is_high(79.9));
        assert!(is_high(80.0));
    }
}
