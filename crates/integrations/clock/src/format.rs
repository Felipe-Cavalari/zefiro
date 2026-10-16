//! Textos do relógio. No Windows vêm do sistema (pt-BR ou o idioma do usuário); `PtBrFallback` serve
//! para testes e para sistemas sem implementação.

pub use crate::grid::Date;
use crate::grid::weekday;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime { pub date: Date, pub hour: u32, pub minute: u32, pub second: u32, pub millis: u32 }

pub trait ClockText {
    /// Validação estrutural compartilhada; a plataforma pode validar também sua API de formato.
    fn validate_format(&self, pattern: &str) -> Result<(), String> { validate_pattern(pattern) }
    /// Texto do chip. `custom_date` vazio = formato do sistema; senão, padrão de data do Windows (ex.: "ddd d MMM").
    fn chip_text(&self, now: &LocalTime, custom_date: &str, seconds: bool) -> String;
    fn long_date(&self, d: Date) -> String;
    fn month_name(&self, year: i32, month: u32) -> String;
    fn weekday_long(&self, d: Date) -> String;
    /// Iniciais dos dias, domingo primeiro.
    fn weekday_initials(&self) -> [String; 7];
    /// 0 = domingo, 1 = segunda…
    fn first_weekday(&self) -> u32;
}

pub fn validate_pattern(pattern: &str) -> Result<(), String> {
    if pattern.contains('\0') { return Err("O formato não pode conter caractere nulo".into()); }
    if pattern.encode_utf16().count() > 1024 { return Err("O formato deve ter no máximo 1024 unidades UTF-16".into()); }
    Ok(())
}

pub struct PtBrFallback;

const DAYS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const SHORT: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const MONTHS: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

impl ClockText for PtBrFallback {
    fn chip_text(&self, now: &LocalTime, _custom_date: &str, seconds: bool) -> String {
        let day = SHORT[weekday(now.date) as usize];
        if seconds {
            format!("{day} {} {:02}:{:02}:{:02}", now.date.day, now.hour, now.minute, now.second)
        } else {
            format!("{day} {} {:02}:{:02}", now.date.day, now.hour, now.minute)
        }
    }
    fn long_date(&self, d: Date) -> String {
        format!("{}, {} de {}", DAYS[weekday(d) as usize], d.day, MONTHS[(d.month - 1) as usize])
    }
    fn month_name(&self, _year: i32, month: u32) -> String {
        MONTHS[(month - 1) as usize].to_string()
    }
    fn weekday_long(&self, d: Date) -> String {
        DAYS[weekday(d) as usize].to_string()
    }
    fn weekday_initials(&self) -> [String; 7] {
        ["D", "S", "T", "Q", "Q", "S", "S"].map(String::from)
    }
    fn first_weekday(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> LocalTime {
        LocalTime { date: Date { year: 2026, month: 10, day: 15 }, hour: 15, minute: 7, second: 9, millis: 0 }
    }

    #[test]
    fn fallback_chip_text() {
        assert_eq!(PtBrFallback.chip_text(&now(), "", false), "qui 15 15:07");
        assert_eq!(PtBrFallback.chip_text(&now(), "", true), "qui 15 15:07:09");
    }

    #[test]
    fn fallback_names() {
        let d = Date { year: 2026, month: 10, day: 15 };
        assert_eq!(PtBrFallback.long_date(d), "quinta-feira, 15 de outubro");
        assert_eq!(PtBrFallback.month_name(2026, 10), "outubro");
        assert_eq!(PtBrFallback.weekday_long(d), "quinta-feira");
        assert_eq!(PtBrFallback.weekday_initials()[0], "D");
        assert_eq!(PtBrFallback.first_weekday(), 0);
    }
}
