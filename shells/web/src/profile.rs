//! 启动档案: 标准入口的 JSON 契约 (D5).
//!
//! //! 严格解析: 固定 schema, 未知键报错, 类型不匹配报错.
//! //! 不用 serde_json (依赖冻结 + AGENTS 反对弱类型 DTO) --
//! //! 这个手写解析器只服务本 schema, golden 全覆盖.

/// 完整启动档案. `pwads` 顺序即加载顺序 (契约的一部分).
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
    //? 计划原稿在这里先吃掉 '}'; 空对象恒报错, 消费与否不可观察, 故省去死赋值.
    if r.peek()? == b'}' {
        return Err("iwad is required".to_string());
    }
    loop {
        let key = r.string()?;
        // 重复键按契约拒绝.
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

/// 严格 JSON 读取器 (只支持本 schema 用到的字面量形态).
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
        let mut out = String::new();
        loop {
            let c = self.b.get(self.i).copied().ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.b.get(self.i).copied().ok_or("bad escape")?;
                    self.i += 1;
                    out.push(match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b't' => '\t',
                        _ => return Err("unsupported escape".to_string()),
                    });
                }
                _ => out.push(c as char),
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
}
