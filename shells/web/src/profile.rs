//! Boot profile: the standard entry's JSON contract (D5).
//!
//! Strict parsing: fixed schema, unknown keys error, type mismatches error.
//! No serde_json (dependency freeze + AGENTS' rejection of weakly-typed DTOs)
//! -- this hand-written parser serves only this schema, with golden coverage.

/// Complete boot profile. `pwads` order is the load order (part of the contract).
///
/// `engine_args` token contract: the host must pre-split every entry into a
/// single argv token -- the shell forwards each element verbatim (one entry
/// stays one argument; `"-warp 1 3"` is never re-split on spaces).
#[derive(Debug, PartialEq, Clone)]
pub struct BootProfile {
    pub iwad: String,
    pub pwads: Vec<String>,
    pub sf2: Option<String>,
    pub max_render_res: Option<u32>,
    pub engine_args: Vec<String>,
}

pub fn parse_profile(json: &str) -> Result<BootProfile, String> {
    let mut r = Reader::new(json);
    let mut p = BootProfile {
        iwad: String::new(),
        pwads: Vec::new(),
        sf2: None,
        max_render_res: None,
        engine_args: Vec::new(),
    };
    r.eat(b'{')?;
    //? The plan draft consumed '}' here first; an empty object always errors
    //? and consuming it is unobservable, so the dead assignment is dropped.
    if r.peek()? == b'}' {
        return Err("iwad is required".to_string());
    }
    loop {
        let key = r.string()?;
        // Duplicate keys are rejected by contract.
        let dup = match key.as_str() {
            "iwad" => !p.iwad.is_empty(),
            "pwads" => !p.pwads.is_empty(),
            "sf2" => p.sf2.is_some(),
            "maxRenderRes" => p.max_render_res.is_some(),
            "engineArgs" => !p.engine_args.is_empty(),
            _ => false,
        };
        if dup {
            return Err(format!("duplicate key: {key}"));
        }
        r.eat(b':')?;
        match key.as_str() {
            "iwad" => p.iwad = r.string()?,
            "pwads" => p.pwads = r.string_array()?,
            "sf2" => p.sf2 = r.string_or_null()?,
            "maxRenderRes" => p.max_render_res = r.u32_or_null()?,
            "engineArgs" => p.engine_args = r.string_array()?,
            other => return Err(format!("unknown key: {other}")),
        }
        match r.peek()? {
            b',' => r.i += 1,
            b'}' => {
                r.i += 1;
                break;
            }
            _ => return Err("expected ',' or '}'".to_string()),
        }
    }
    r.ws();
    if r.i != r.b.len() {
        return Err("trailing characters".to_string());
    }
    if p.iwad.is_empty() {
        return Err("iwad is required".to_string());
    }
    Ok(p)
}

/// Strict JSON reader (supports only the literal forms this schema uses).
struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            b: s.as_bytes(),
            i: 0,
        }
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Result<u8, String> {
        self.ws();
        self.b
            .get(self.i)
            .copied()
            .ok_or_else(|| "unexpected end".to_string())
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        if self.peek()? == c {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}'", c as char))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        // Collect raw bytes and validate UTF-8 at the end: a per-byte `c as char`
        // would silently mojibake multi-byte characters (Latin-1 reinterpretation).
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.b.get(self.i).copied().ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => {
                    return String::from_utf8(out)
                        .map_err(|_| "string is not valid UTF-8".to_string())
                }
                b'\\' => {
                    let e = self.b.get(self.i).copied().ok_or("bad escape")?;
                    self.i += 1;
                    out.push(match e {
                        b'"' => b'"',
                        b'\\' => b'\\',
                        b'/' => b'/',
                        b'n' => b'\n',
                        b't' => b'\t',
                        _ => return Err("unsupported escape".to_string()),
                    });
                }
                // A raw NUL (e.g. the JS host passing 'a\u0000b' as a literal) is
                // never legitimate in a profile value (filenames/argv): reject it
                // here so no interior-NUL string can reach build_argv or the VFS.
                0 => return Err("NUL byte in string value".to_string()),
                _ => out.push(c),
            }
        }
    }

    fn u32_or_null(&mut self) -> Result<Option<u32>, String> {
        self.ws();
        if self.b[self.i..].starts_with(b"null") {
            self.i += 4;
            return Ok(None);
        }
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if start == self.i {
            return Err("expected number or null".to_string());
        }
        std::str::from_utf8(&self.b[start..self.i])
            .unwrap()
            .parse::<u32>()
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn string_or_null(&mut self) -> Result<Option<String>, String> {
        self.ws();
        if self.b[self.i..].starts_with(b"null") {
            self.i += 4;
            return Ok(None);
        }
        self.string().map(Some)
    }

    fn string_array(&mut self) -> Result<Vec<String>, String> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        if self.peek()? == b']' {
            self.i += 1;
            return Ok(out);
        }
        loop {
            out.push(self.string()?);
            match self.peek()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => Err("expected ',' or ']'".to_string())?,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_profile() {
        let p = parse_profile(
            r#"{"iwad":"doom1.wad","pwads":["a.wad","b.wad"],"sf2":"sc55.sf2","maxRenderRes":1080,"engineArgs":["-nomusic","-turbo 2"]}"#,
        )
        .unwrap();
        assert_eq!(p.iwad, "doom1.wad");
        assert_eq!(p.pwads, vec!["a.wad".to_string(), "b.wad".to_string()]);
        assert_eq!(p.sf2.as_deref(), Some("sc55.sf2"));
        assert_eq!(p.max_render_res, Some(1080));
        assert_eq!(
            p.engine_args,
            vec!["-nomusic".to_string(), "-turbo 2".to_string()]
        );
    }

    #[test]
    fn minimal_profile_defaults() {
        let p = parse_profile(r#"{"iwad":"doom.wad"}"#).unwrap();
        assert_eq!(p.iwad, "doom.wad");
        assert!(p.pwads.is_empty());
        assert_eq!(p.sf2, None);
        assert_eq!(p.max_render_res, None);
        assert!(p.engine_args.is_empty());
    }

    #[test]
    fn explicit_nulls_are_none() {
        let p = parse_profile(r#"{"iwad":"d.wad","sf2":null,"maxRenderRes":null}"#).unwrap();
        assert_eq!(p.sf2, None);
        assert_eq!(p.max_render_res, None);
    }

    #[test]
    fn unknown_key_rejected() {
        assert!(parse_profile(r#"{"iwad":"d.wad","cheat":1}"#).is_err());
    }

    #[test]
    fn missing_iwad_rejected() {
        assert!(parse_profile(r#"{"pwads":[]}"#).is_err());
    }

    #[test]
    fn wrong_value_type_rejected() {
        assert!(parse_profile(r#"{"iwad":42}"#).is_err());
        assert!(parse_profile(r#"{"pwads":"a.wad"}"#).is_err());
        assert!(parse_profile(r#"{"maxRenderRes":"big"}"#).is_err());
    }

    #[test]
    fn trailing_garbage_rejected() {
        assert!(parse_profile(r#"{"iwad":"d.wad"} oops"#).is_err());
        assert!(parse_profile("").is_err());
    }

    #[test]
    fn duplicate_key_rejected() {
        assert!(parse_profile(r#"{"iwad":"a.wad","iwad":"b.wad"}"#).is_err());
    }

    #[test]
    fn raw_nul_in_string_value_rejected_not_panic() {
        // The JS host can hand over a raw U+0000 (a JS '\u0000' escape lands as a
        // literal NUL byte inside the &str). Must be a clean Err, never a panic.
        let json = format!(r#"{{"iwad":"a{}b.wad"}}"#, '\u{0}');
        let err = parse_profile(&json).unwrap_err();
        assert!(err.contains("NUL"), "unexpected error: {err}");
    }

    #[test]
    fn utf8_string_values_decode_properly() {
        // Regression for the round-0 mojibake: non-ASCII bytes inside a JSON
        // string must decode as UTF-8, not be reinterpreted byte-wise as Latin-1.
        let p = parse_profile("{\"iwad\":\"wäd.wad\"}").unwrap();
        assert_eq!(p.iwad, "wäd.wad");
    }
}
