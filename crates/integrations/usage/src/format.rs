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

/// Dias desde 1970-01-01 de uma data civil (algoritmo `days_from_civil` de Howard Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// `2026-10-16T02:56:30.609Z` → segundos Unix. Só o formato UTC com `Z` que o Codex grava.
pub fn parse_iso_utc(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.split('.').next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, m, sec) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || h > 23 || m > 59 || sec > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + m * 60 + sec)
}

/// "15/10 23:56": `unix` (UTC) no fuso `utc_offset_s`.
pub fn format_short(unix: i64, utc_offset_s: i32) -> String {
    let local = unix + i64::from(utc_offset_s);
    let (days, secs) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (month, day) = month_day(days);
    format!("{day:02}/{month:02} {:02}:{:02}", secs / 3600, secs % 3600 / 60)
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

    #[test]
    fn iso_timestamps_and_short_dates() {
        assert_eq!(parse_iso_utc("2026-10-16T02:56:30.609Z"), Some(1792119390));
        assert_eq!(parse_iso_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso_utc("ontem"), None);
        // 02:56 UTC do dia 16 é 23:56 do dia 15 em Brasília.
        assert_eq!(format_short(1792119390, -10800), "15/10 23:56");
    }
}
