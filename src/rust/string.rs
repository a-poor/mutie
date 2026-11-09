//! Canonical str-literal recipes, without macro expansion or source evaluation.

fn physical_newlines(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .enumerate()
        .all(|(i, b)| *b != b'\r' || bytes.get(i + 1) == Some(&b'\n'))
}

pub(super) fn replacement(original: &str) -> Option<&'static str> {
    let bytes = original.as_bytes();
    let nonempty = if bytes.first() == Some(&b'r') {
        let hashes = bytes[1..].iter().take_while(|b| **b == b'#').count();
        if hashes > 255 || bytes.get(1 + hashes) != Some(&b'"') {
            return None;
        }
        let close = format!("\"{}", "#".repeat(hashes));
        let body = original.get(hashes + 2..)?;
        let end = body.find(&close)?;
        if end + close.len() != body.len() || !physical_newlines(&body.as_bytes()[..end]) {
            return None;
        }
        end != 0
    } else {
        if bytes.first() != Some(&b'"') {
            return None;
        }
        let mut i = 1;
        let mut nonempty = false;
        loop {
            let byte = *bytes.get(i)?;
            i += 1;
            match byte {
                b'"' => {
                    if i != bytes.len() {
                        return None;
                    }
                    break;
                }
                b'\\' => {
                    let escape = *bytes.get(i)?;
                    i += 1;
                    if escape == b'\n' || (escape == b'\r' && bytes.get(i) == Some(&b'\n')) {
                        // Rust removes the following HT/LF/CR/space as well.
                        while matches!(bytes.get(i), Some(b'\t' | b'\n' | b'\r' | b' ')) {
                            i += 1;
                        }
                        continue;
                    }
                    match escape {
                        b'0' | b'n' | b'r' | b't' | b'\\' | b'\'' | b'"' => {}
                        b'x' => {
                            if !matches!(bytes.get(i), Some(b'0'..=b'7'))
                                || !bytes.get(i + 1)?.is_ascii_hexdigit()
                            {
                                return None;
                            }
                            i += 2;
                        }
                        b'u' => {
                            if bytes.get(i) != Some(&b'{') || !bytes.get(i + 1)?.is_ascii_hexdigit()
                            {
                                return None;
                            }
                            i += 1;
                            let mut count = 0;
                            let mut scalar = 0;
                            while let Some(b) = bytes.get(i) {
                                if *b == b'_' {
                                    i += 1;
                                    continue;
                                }
                                let Some(digit) = (*b as char).to_digit(16) else {
                                    break;
                                };
                                count += 1;
                                if count > 6 {
                                    return None;
                                }
                                scalar = scalar * 16 + digit;
                                i += 1;
                            }
                            if bytes.get(i) != Some(&b'}') || char::from_u32(scalar).is_none() {
                                return None;
                            }
                            i += 1;
                        }
                        _ => return None,
                    }
                }
                b'\r' if bytes.get(i) != Some(&b'\n') => return None,
                _ => {}
            }
            nonempty = true;
        }
        nonempty
    };
    Some(if nonempty { "\"\"" } else { "\"x\"" })
}

pub(crate) fn valid_replacement(original: &str, changed: &str) -> bool {
    replacement(original) == Some(changed)
}
