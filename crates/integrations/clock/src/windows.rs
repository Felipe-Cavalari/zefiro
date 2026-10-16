//! Textos do relógio pelo Windows (idioma e formatos do usuário).

use crate::format::{ClockText, LocalTime};
use crate::grid::Date;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::*;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::{HSTRING, PCWSTR};

/// `LOCALE_NAME_USER_DEFAULT` (NULL no SDK; a crate windows não exporta a constante).
const USER_DEFAULT: PCWSTR = PCWSTR::null();

pub struct WindowsClock;

fn systemtime(d: Date, hour: u32, minute: u32, second: u32) -> SYSTEMTIME {
    SYSTEMTIME {
        wYear: d.year as u16,
        wMonth: d.month as u16,
        wDay: d.day as u16,
        wDayOfWeek: crate::grid::weekday(d) as u16,
        wHour: hour as u16,
        wMinute: minute as u16,
        wSecond: second as u16,
        wMilliseconds: 0,
    }
}

fn date_text(st: &SYSTEMTIME, pattern: &str) -> String {
    let pat = HSTRING::from(pattern);
    // SAFETY: consulta o tamanho necessário sem fornecer buffer de saída.
    let size = unsafe { GetDateFormatEx(USER_DEFAULT, ENUM_DATE_FORMATS_FLAGS(0), Some(st), &pat, None, None) };
    if size <= 0 { return String::new(); }
    let mut buf = vec![0u16; size as usize];
    // SAFETY: buffer local com tamanho correto; padrão terminado em zero (HSTRING).
    let n = unsafe { GetDateFormatEx(USER_DEFAULT, ENUM_DATE_FORMATS_FLAGS(0), Some(st), &pat, Some(&mut buf), None) };
    String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
}

fn time_text(st: &SYSTEMTIME, seconds: bool) -> String {
    let mut buf = [0u16; 64];
    let flags = if seconds { TIME_FORMAT_FLAGS(0) } else { TIME_NOSECONDS };
    // SAFETY: buffer local; formato nulo = formato curto do usuário.
    let n = unsafe { GetTimeFormatEx(USER_DEFAULT, flags, Some(st), PCWSTR::null(), Some(&mut buf)) };
    String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize])
}

impl ClockText for WindowsClock {
    fn validate_format(&self, pattern: &str) -> Result<(), String> {
        crate::format::validate_pattern(pattern)?;
        if pattern.is_empty() { return Ok(()); }
        let pattern = HSTRING::from(pattern);
        let st = systemtime(Date { year: 2026, month: 10, day: 15 }, 12, 0, 0);
        // SAFETY: data válida e string UTF-16 local; apenas consulta tamanho.
        let size = unsafe { GetDateFormatEx(USER_DEFAULT, ENUM_DATE_FORMATS_FLAGS(0), Some(&st), &pattern, None, None) };
        if size > 0 { Ok(()) } else { Err("O Windows não aceitou o formato de data".into()) }
    }
    fn chip_text(&self, now: &LocalTime, custom_date: &str, seconds: bool) -> String {
        let st = systemtime(now.date, now.hour, now.minute, now.second);
        let date = date_text(&st, if custom_date.is_empty() { "ddd d" } else { custom_date });
        format!("{date} {}", time_text(&st, seconds))
    }
    fn long_date(&self, d: Date) -> String {
        date_text(&systemtime(d, 0, 0, 0), "dddd, d 'de' MMMM")
    }
    fn month_name(&self, year: i32, month: u32) -> String {
        date_text(&systemtime(Date { year, month, day: 1 }, 0, 0, 0), "MMMM")
    }
    fn weekday_long(&self, d: Date) -> String {
        date_text(&systemtime(d, 0, 0, 0), "dddd")
    }
    fn weekday_initials(&self) -> [String; 7] {
        std::array::from_fn(|weekday| {
            // CAL_SSHORTESTDAYNAME1 é segunda; o índice 0 aqui é domingo.
            let cal = CAL_SSHORTESTDAYNAME1 + ((weekday as u32 + 6) % 7);
            let mut buf = [0u16; 16];
            // SAFETY: buffer local com tamanho correto.
            let n = unsafe { GetCalendarInfoEx(USER_DEFAULT, CAL_GREGORIAN, PCWSTR::null(), cal, Some(&mut buf), None) };
            if n > 1 { String::from_utf16_lossy(&buf[..(n - 1) as usize]) } else { ["D", "S", "T", "Q", "Q", "S", "S"][weekday].to_string() }
        })
    }
    fn first_weekday(&self) -> u32 {
        let mut value = [0u16; 2];
        // SAFETY: LOCALE_RETURN_NUMBER grava um DWORD (2 u16) no buffer local.
        let n = unsafe { GetLocaleInfoEx(USER_DEFAULT, LOCALE_IFIRSTDAYOFWEEK | LOCALE_RETURN_NUMBER, Some(&mut value)) };
        let raw = u32::from(value[0]) | (u32::from(value[1]) << 16);
        // O Windows conta 0 = segunda.
        if n > 0 { (raw + 1) % 7 } else { 0 }
    }
}

pub fn now_local() -> LocalTime {
    // SAFETY: sem parâmetros.
    let st = unsafe { GetLocalTime() };
    LocalTime {
        date: Date { year: st.wYear as i32, month: st.wMonth as u32, day: st.wDay as u32 },
        hour: st.wHour as u32,
        minute: st.wMinute as u32,
        second: st.wSecond as u32,
        millis: st.wMilliseconds as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_texts_are_not_empty() {
        let c = WindowsClock;
        let now = now_local();
        assert!(!c.chip_text(&now, "", false).is_empty());
        assert!(!c.long_date(now.date).is_empty());
        assert!(c.weekday_initials().iter().all(|s| !s.is_empty()));
        assert!(c.first_weekday() < 7);
    }
}
