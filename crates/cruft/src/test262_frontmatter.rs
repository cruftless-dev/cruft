
use std::collections::BTreeSet;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Negative {
    pub phase: Option<String>,
    pub r#type: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Frontmatter {

    pub flags: BTreeSet<String>,
    pub includes: Vec<String>,
    pub features: Vec<String>,
    pub negative: Option<Negative>,
    pub description: String,
}

impl Frontmatter {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
}

fn parse_inline_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Vec::new();
    }

    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

pub fn parse_frontmatter(src: &str) -> Frontmatter {
    let mut meta = Frontmatter::default();

    let body = match src.find("/*---") {
        None => return meta,
        Some(start) => {
            let rest = &src[start + 5..];
            match rest.find("---*/") {
                None => return meta,
                Some(end) => &rest[..end],
            }
        }
    };

    let normalized = body
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{2028}', "\n")
        .replace('\u{2029}', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("flags:") {
            for f in parse_inline_array(rest.trim()) {
                meta.flags.insert(f);
            }
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("includes:") {
            meta.includes = parse_inline_array(rest.trim());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("features:") {
            meta.features = parse_inline_array(rest.trim());
            i += 1;
            continue;
        }
        if trimmed.starts_with("negative:") {

            let mut neg = Negative::default();
            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                if !sub.starts_with("  ") && !sub.starts_with('\t') {
                    break;
                }
                let st = sub.trim();
                match st.find(':') {
                    None => {
                        i += 1;
                        continue;
                    }
                    Some(colon) => {
                        let k = st[..colon].trim();
                        let v = st[colon + 1..].trim();
                        if k == "phase" {
                            neg.phase = Some(v.to_string());
                        } else if k == "type" {
                            neg.r#type = Some(v.to_string());
                        }
                        i += 1;
                    }
                }
            }
            meta.negative = Some(neg);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("description:") {
            meta.description = rest.trim().to_string();
            i += 1;
            continue;
        }
        i += 1;
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(s: &str) -> Frontmatter {
        parse_frontmatter(s)
    }

    #[test]
    fn no_frontmatter_yields_empty() {
        let m = fm("var x = 1;\n");
        assert!(m.flags.is_empty());
        assert!(m.includes.is_empty());
        assert!(m.features.is_empty());
        assert!(m.negative.is_none());
        assert_eq!(m.description, "");
    }

    #[test]
    fn flags_inline_array() {
        let m = fm("/*---\nflags: [module, async]\n---*/\n");
        assert!(m.has_flag("module"));
        assert!(m.has_flag("async"));
        assert_eq!(m.flags.len(), 2);
    }

    #[test]
    fn includes_and_features() {
        let m =
            fm("/*---\nincludes: [sta.js, assert.js]\nfeatures: [BigInt, Symbol.iterator]\n---*/");
        assert_eq!(m.includes, vec!["sta.js", "assert.js"]);
        assert_eq!(m.features, vec!["BigInt", "Symbol.iterator"]);
    }

    #[test]
    fn negative_subblock() {
        let m = fm("/*---\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/");
        let n = m.negative.expect("negative present");
        assert_eq!(n.phase.as_deref(), Some("parse"));
        assert_eq!(n.r#type.as_deref(), Some("SyntaxError"));
    }

    #[test]
    fn negative_with_tab_indent() {
        let m = fm("/*---\nnegative:\n\tphase: runtime\n\ttype: TypeError\n---*/");
        let n = m.negative.expect("negative present");
        assert_eq!(n.phase.as_deref(), Some("runtime"));
        assert_eq!(n.r#type.as_deref(), Some("TypeError"));
    }

    #[test]
    fn negative_subblock_ends_at_dedent() {

        let m = fm("/*---\nnegative:\n  phase: parse\n  type: SyntaxError\nflags: [raw]\n---*/");
        assert!(m.has_flag("raw"));
        assert_eq!(m.negative.unwrap().phase.as_deref(), Some("parse"));
    }

    #[test]
    fn comments_and_blank_lines_skipped() {
        let m = fm("/*---\n# a comment\n\nflags: [onlyStrict]\n---*/");
        assert!(m.has_flag("onlyStrict"));
        assert_eq!(m.flags.len(), 1);
    }

    #[test]
    fn empty_array_and_non_array() {
        assert!(fm("/*---\nfeatures: []\n---*/").features.is_empty());

        assert!(fm("/*---\nfeatures:\n  - a\n  - b\n---*/")
            .features
            .is_empty());
    }

    #[test]
    fn description_inline() {
        let m = fm("/*---\ndescription: Compare NaN with NaN\n---*/");
        assert_eq!(m.description, "Compare NaN with NaN");
    }

    #[test]
    fn first_marker_pair_is_used_nongreedy() {

        let m = fm("/*---\nflags: [module]\n---*/\nvar x;\n/*--- not frontmatter ---*/");
        assert!(m.has_flag("module"));
        assert_eq!(m.flags.len(), 1);
    }

    #[test]
    fn inline_array_tight_spacing() {
        let m = fm("/*---\nincludes: [a.js,b.js, c.js]\n---*/");
        assert_eq!(m.includes, vec!["a.js", "b.js", "c.js"]);
    }

    #[test]
    #[ignore]
    fn emit_corpus_for_crosscheck() {
        let paths_file = match std::env::var("FM_PATHS") {
            Ok(p) => p,
            Err(_) => return,
        };
        let out_file = std::env::var("FM_OUT").expect("FM_OUT set");
        let listing = std::fs::read_to_string(&paths_file).expect("read FM_PATHS");
        let mut buf = String::new();
        for path in listing.lines().filter(|l| !l.trim().is_empty()) {
            let src = std::fs::read_to_string(path).unwrap_or_default();
            let m = parse_frontmatter(&src);
            buf.push_str(path);
            buf.push('\t');
            buf.push_str(&canonical_json(&m));
            buf.push('\n');
        }
        std::fs::write(&out_file, buf).expect("write FM_OUT");
    }

    fn json_str_array(items: &[String]) -> String {
        let parts: Vec<String> = items.iter().map(|s| json_string(s)).collect();
        format!("[{}]", parts.join(","))
    }

    fn json_string(s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(c),
            }
        }
        out.push('"');
        out
    }

    fn canonical_json(m: &Frontmatter) -> String {
        let flags: Vec<String> = m.flags.iter().cloned().collect();
        let neg = match &m.negative {
            None => "null".to_string(),
            Some(n) => format!(
                "{{\"phase\":{},\"type\":{}}}",
                n.phase
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| "null".into()),
                n.r#type
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| "null".into()),
            ),
        };
        format!(
            "{{\"flags\":{},\"includes\":{},\"features\":{},\"negative\":{}}}",
            json_str_array(&flags),
            json_str_array(&m.includes),
            json_str_array(&m.features),
            neg
        )
    }
}
