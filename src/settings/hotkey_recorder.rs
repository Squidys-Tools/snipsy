use gpui::Keystroke;

/// gpui reports Shift+digit / Shift+punctuation as the shifted character with
/// `shift` cleared; map it back to the physical (US layout) key.
fn unshift(c: &str) -> Option<&'static str> {
    Some(match c {
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        "_" => "-",
        "+" => "=",
        "{" => "[",
        "}" => "]",
        "|" => "\\",
        ":" => ";",
        "\"" => "'",
        "<" => ",",
        ">" => ".",
        "?" => "/",
        "~" => "`",
        _ => return None,
    })
}

pub(super) fn accel_from(ks: &Keystroke) -> Option<String> {
    let mut m = ks.modifiers.clone();
    let k = match unshift(ks.key.as_str()) {
        Some(base) => {
            m.shift = true;
            base
        }
        None => ks.key.as_str(),
    };
    let code = match k {
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_alphabetic() => format!("Key{}", k.to_ascii_uppercase()),
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_digit() => format!("Digit{k}"),
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok() => k.to_ascii_uppercase(),
        "space" => "Space".into(),
        "-" => "Minus".into(),
        "=" => "Equal".into(),
        "[" => "BracketLeft".into(),
        "]" => "BracketRight".into(),
        ";" => "Semicolon".into(),
        "'" => "Quote".into(),
        "," => "Comma".into(),
        "." => "Period".into(),
        "/" => "Slash".into(),
        "\\" => "Backslash".into(),
        "`" => "Backquote".into(),
        _ => return None,
    };
    let fkey = code.starts_with('F') && code.len() <= 3;
    if !(m.control || m.alt || m.shift || m.platform) && !fkey {
        return None;
    }
    let mut parts = vec![];
    if m.control {
        parts.push("ctrl".to_string());
    }
    if m.alt {
        parts.push("alt".to_string());
    }
    if m.shift {
        parts.push("shift".to_string());
    }
    if m.platform {
        parts.push("super".to_string());
    }
    parts.push(code);
    Some(parts.join("+"))
}
