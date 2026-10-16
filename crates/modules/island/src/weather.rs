//! Clima do wttr.in (`?format=j1`), como a ilha C++: leitura atual + 3 dias. Portável: só o
//! endereço, a leitura do JSON e o ícone por código do World Weather Online.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Day { pub max_c: i32, pub min_c: i32, pub code: u32 }

#[derive(Debug, Clone, PartialEq)]
pub struct Weather {
    pub city: String,
    pub temp_c: i32,
    pub feels_like_c: i32,
    pub humidity: u32,
    pub wind_kmh: u32,
    pub description: String,
    pub code: u32,
    pub days: Vec<Day>,
}

/// Endereço da consulta: cidade da configuração ou, vazio, a do IP (o wttr.in decide).
pub fn url(city: &str) -> String {
    let encoded: String = city.trim().chars().map(|c| if c == ' ' { "+".to_string() } else { c.to_string() }).collect();
    format!("https://wttr.in/{encoded}?format=j1&lang=pt")
}

#[derive(Deserialize)]
struct Value { value: String }

#[derive(Deserialize)]
struct Current {
    #[serde(rename = "temp_C")]
    temp_c: String,
    #[serde(rename = "FeelsLikeC")]
    feels_like_c: String,
    humidity: String,
    #[serde(rename = "windspeedKmph")]
    wind_kmph: String,
    #[serde(rename = "weatherCode")]
    code: String,
    #[serde(rename = "weatherDesc", default)]
    desc: Vec<Value>,
    #[serde(rename = "lang_pt", default)]
    desc_pt: Vec<Value>,
}

#[derive(Deserialize)]
struct Hourly {
    #[serde(rename = "weatherCode")]
    code: String,
}

#[derive(Deserialize)]
struct DayJson {
    #[serde(rename = "maxtempC")]
    max_c: String,
    #[serde(rename = "mintempC")]
    min_c: String,
    #[serde(default)]
    hourly: Vec<Hourly>,
}

#[derive(Deserialize)]
struct Area {
    #[serde(rename = "areaName", default)]
    name: Vec<Value>,
}

#[derive(Deserialize)]
struct Root {
    #[serde(default)]
    current_condition: Vec<Current>,
    #[serde(default)]
    nearest_area: Vec<Area>,
    #[serde(default)]
    weather: Vec<DayJson>,
}

/// `None` quando a resposta não tem leitura (limite de uso ou cidade desconhecida). Código 0 não
/// existe no WWO: sem código e sem descrição, não aceita (era o "0 grau" falso da ilha C++).
pub fn parse(json: &str, city_override: &str) -> Option<Weather> {
    let root: Root = serde_json::from_str(json).ok()?;
    let c = root.current_condition.into_iter().next()?;
    let num = |s: &str| s.trim().parse::<i32>().ok();
    let code = c.code.trim().parse::<u32>().unwrap_or(0);
    let description = c.desc_pt.first().or(c.desc.first()).map(|v| v.value.trim().to_string()).unwrap_or_default();
    if code == 0 && description.is_empty() {
        return None;
    }
    let city = if city_override.trim().is_empty() { root.nearest_area.first().and_then(|a| a.name.first()).map(|v| v.value.clone()).unwrap_or_default() } else { city_override.trim().to_string() };
    let days = root
        .weather
        .iter()
        .take(3)
        .filter_map(|d| {
            // O ícone do dia é o do meio-dia (4ª de 8 leituras de 3 h), ou a primeira que houver.
            let code = d.hourly.get(4).or(d.hourly.first()).and_then(|h| h.code.trim().parse().ok()).unwrap_or(113);
            Some(Day { max_c: num(&d.max_c)?, min_c: num(&d.min_c)?, code })
        })
        .collect();
    Some(Weather {
        city,
        temp_c: num(&c.temp_c)?,
        feels_like_c: num(&c.feels_like_c).unwrap_or(0),
        humidity: c.humidity.trim().parse().unwrap_or(0),
        wind_kmh: c.wind_kmph.trim().parse().unwrap_or(0),
        description,
        code,
        days,
    })
}

/// Emoji do código do World Weather Online (desenhado com a fonte colorida do Windows).
pub fn icon(code: u32) -> &'static str {
    match code {
        113 => "☀️",
        116 => "⛅",
        119 | 122 => "☁️",
        143 | 248 | 260 => "🌫️",
        200 | 386 | 389 | 392 | 395 => "⛈️",
        176 | 263 | 266 | 293 | 296 | 353 => "🌦️",
        299..=314 | 356 | 359 => "🌧️",
        179 | 182 | 185 | 227 | 230 | 317..=338 | 350 | 362..=377 => "🌨️",
        _ => "🌡️",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "current_condition": [{"temp_C": "21", "FeelsLikeC": "21", "humidity": "89", "windspeedKmph": "9",
        "weatherCode": "266", "weatherDesc": [{"value": "Light drizzle"}], "lang_pt": [{"value": "Chuvisco leve"}]}],
      "nearest_area": [{"areaName": [{"value": "Guarulhos"}]}],
      "weather": [
        {"maxtempC": "24", "mintempC": "17", "hourly": [{"weatherCode":"113"},{"weatherCode":"113"},{"weatherCode":"113"},{"weatherCode":"113"},{"weatherCode":"296"}]},
        {"maxtempC": "27", "mintempC": "18", "hourly": [{"weatherCode":"116"}]},
        {"maxtempC": "29", "mintempC": "19", "hourly": []}
      ]
    }"#;

    #[test]
    fn parses_current_reading_area_and_three_days() {
        let w = parse(SAMPLE, "").unwrap();
        assert_eq!((w.city.as_str(), w.temp_c, w.humidity, w.wind_kmh), ("Guarulhos", 21, 89, 9));
        assert_eq!(w.description, "Chuvisco leve");
        assert_eq!(w.days, vec![Day { max_c: 24, min_c: 17, code: 296 }, Day { max_c: 27, min_c: 18, code: 116 }, Day { max_c: 29, min_c: 19, code: 113 }]);
        assert_eq!(parse(SAMPLE, " São Paulo ").unwrap().city, "São Paulo");
    }

    #[test]
    fn empty_or_unusable_answers_are_rejected() {
        assert_eq!(parse("<html>limite</html>", ""), None);
        assert_eq!(parse(r#"{"current_condition": []}"#, ""), None);
        assert_eq!(parse(r#"{"current_condition": [{"temp_C":"0","FeelsLikeC":"0","humidity":"0","windspeedKmph":"0","weatherCode":"0"}]}"#, ""), None);
    }

    #[test]
    fn url_and_icons() {
        assert_eq!(url(""), "https://wttr.in/?format=j1&lang=pt");
        assert_eq!(url("Rio de Janeiro"), "https://wttr.in/Rio+de+Janeiro?format=j1&lang=pt");
        assert_eq!(icon(113), "☀️");
        assert_eq!(icon(302), "🌧️");
        assert_eq!(icon(1), "🌡️");
    }
}
