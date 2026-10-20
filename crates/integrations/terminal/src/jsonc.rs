//! JSON com comentários e vírgulas sobrando, como o `settings.json` do Windows Terminal.

/// Tira o BOM, os comentários (`//` e `/* */`) e as vírgulas antes de `]` ou `}`, sem mexer no que
/// está dentro de strings.
pub fn strip(text: &str) -> String {
    drop_trailing_commas(&drop_comments(text.trim_start_matches('\u{feff}')))
}

fn drop_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut star = false;
                for c in chars.by_ref() {
                    if star && c == '/' {
                        break;
                    }
                    star = c == '*';
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

/// Sem comentários: uma vírgula fora de string seguida só de espaço e `]`/`}` sai.
fn drop_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' && chars[i + 1..].iter().find(|c| !c.is_whitespace()).is_some_and(|c| matches!(c, ']' | '}')) {
            // vírgula sobrando
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_keeps_slashes_inside_strings() {
        let t = "{\"a\": \"ssh //host /* x */\", // c\n \"b\": \"\\\"q\\\"\", /* d */ \"c\": [1,2,],}";
        let v: serde_json::Value = serde_json::from_str(&strip(t)).unwrap();
        assert_eq!(v["a"], "ssh //host /* x */");
        assert_eq!(v["b"], "\"q\"");
        assert_eq!(v["c"], serde_json::json!([1, 2]));
    }

    #[test]
    fn strip_drops_bom() {
        assert!(serde_json::from_str::<serde_json::Value>(&strip("\u{feff}{}")).is_ok());
    }
}
