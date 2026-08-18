
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,

    pub pre: Vec<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identifier {
    Numeric(u64),
    AlphaNumeric(String),
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Version {
            major,
            minor,
            patch,
            pre: Vec::new(),
        }
    }

    pub fn parse(s: &str) -> Result<Version, SemverError> {
        let s = s.trim();
        let s = s
            .strip_prefix('v')
            .or_else(|| s.strip_prefix('='))
            .unwrap_or(s)
            .trim();

        let (s, _build) = match s.split_once('+') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };
        let (core, pre_str) = match s.split_once('-') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };
        let mut it = core.split('.');
        let major = parse_num(it.next())?;
        let minor = parse_num(it.next())?;
        let patch = parse_num(it.next())?;
        if it.next().is_some() {
            return Err(SemverError::Parse(format!(
                "too many version segments: {s}"
            )));
        }
        let pre = match pre_str {
            None => Vec::new(),
            Some(p) if p.is_empty() => return Err(SemverError::Parse("empty prerelease".into())),
            Some(p) => parse_pre(p)?,
        };
        Ok(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn cmp_precedence(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| cmp_pre(&self.pre, &other.pre))
    }

    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp_precedence(other))
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.cmp_precedence(other)
    }
}

fn cmp_pre(a: &[Identifier], b: &[Identifier]) -> Ordering {

    match (a.is_empty(), b.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let o = match (x, y) {
                    (Identifier::Numeric(m), Identifier::Numeric(n)) => m.cmp(n),

                    (Identifier::Numeric(_), Identifier::AlphaNumeric(_)) => Ordering::Less,
                    (Identifier::AlphaNumeric(_), Identifier::Numeric(_)) => Ordering::Greater,
                    (Identifier::AlphaNumeric(m), Identifier::AlphaNumeric(n)) => m.cmp(n),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }

            a.len().cmp(&b.len())
        }
    }
}

fn parse_num(seg: Option<&str>) -> Result<u64, SemverError> {
    let s = seg.ok_or_else(|| SemverError::Parse("missing version segment".into()))?;
    s.parse::<u64>()
        .map_err(|_| SemverError::Parse(format!("non-numeric version segment: {s}")))
}

fn checked_inc(n: u64, ctx: &str) -> Result<u64, SemverError> {
    n.checked_add(1)
        .ok_or_else(|| SemverError::Parse(format!("version component overflow in {ctx}")))
}

fn parse_pre(p: &str) -> Result<Vec<Identifier>, SemverError> {
    let mut out = Vec::new();
    for id in p.split('.') {
        if id.is_empty() {
            return Err(SemverError::Parse("empty prerelease identifier".into()));
        }

        if id.bytes().all(|b| b.is_ascii_digit()) {
            if id.len() > 1 && id.starts_with('0') {
                return Err(SemverError::Parse(format!(
                    "leading-zero numeric prerelease: {id}"
                )));
            }
            let n = id
                .parse::<u64>()
                .map_err(|_| SemverError::Parse(format!("numeric prerelease overflow: {id}")))?;
            out.push(Identifier::Numeric(n));
        } else {
            out.push(Identifier::AlphaNumeric(id.to_string()));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
}

#[derive(Debug, Clone)]
pub struct Comparator {
    pub op: Op,
    pub version: Version,
}

impl Comparator {
    fn matches(&self, v: &Version) -> bool {
        let o = v.cmp_precedence(&self.version);
        match self.op {
            Op::Lt => o == Ordering::Less,
            Op::Lte => o != Ordering::Greater,
            Op::Gt => o == Ordering::Greater,
            Op::Gte => o != Ordering::Less,
            Op::Eq => o == Ordering::Equal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComparatorSet {
    pub comparators: Vec<Comparator>,
}

#[derive(Debug, Clone)]
pub struct Range {
    pub sets: Vec<ComparatorSet>,
}

#[derive(Debug)]
pub enum SemverError {
    Parse(String),
}

impl std::fmt::Display for SemverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemverError::Parse(s) => write!(f, "semver parse error: {s}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Partial {
    major: Option<u64>,
    minor: Option<u64>,
    patch: Option<u64>,
}

fn parse_partial(s: &str) -> Result<(Partial, Vec<Identifier>), SemverError> {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let (s, _build) = match s.split_once('+') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    let (core, pre_str) = match s.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    let mut parts = core.split('.');
    let major = parse_xr(parts.next())?;
    let minor = parse_xr(parts.next())?;
    let patch = parse_xr(parts.next())?;
    let pre = match pre_str {
        None => Vec::new(),
        Some(p) if p.is_empty() => Vec::new(),
        Some(p) => parse_pre(p)?,
    };
    Ok((
        Partial {
            major,
            minor,
            patch,
        },
        pre,
    ))
}

fn parse_xr(seg: Option<&str>) -> Result<Option<u64>, SemverError> {
    match seg {
        None => Ok(None),
        Some(s) => {
            let s = s.trim();
            if s.is_empty() || s == "x" || s == "X" || s == "*" {
                Ok(None)
            } else {
                s.parse::<u64>()
                    .map(Some)
                    .map_err(|_| SemverError::Parse(format!("bad version segment: {s}")))
            }
        }
    }
}

impl Range {

    pub fn parse(input: &str) -> Result<Range, SemverError> {
        let input = input.trim();
        let mut sets = Vec::new();
        for clause in input.split("||") {
            sets.push(parse_comparator_set(clause.trim())?);
        }
        if sets.is_empty() {
            sets.push(ComparatorSet {
                comparators: vec![],
            });
        }
        Ok(Range { sets })
    }

    pub fn satisfies(&self, v: &Version) -> bool {
        self.sets.iter().any(|set| set_satisfies(set, v))
    }
}

fn set_satisfies(set: &ComparatorSet, v: &Version) -> bool {
    if !set.comparators.iter().all(|c| c.matches(v)) {
        return false;
    }
    if v.is_prerelease() {

        let allowed = set.comparators.iter().any(|c| {
            !c.version.pre.is_empty()
                && c.version.major == v.major
                && c.version.minor == v.minor
                && c.version.patch == v.patch
        });
        if !allowed {
            return false;
        }
    }
    true
}

fn parse_comparator_set(clause: &str) -> Result<ComparatorSet, SemverError> {
    let clause = clause.trim();

    if clause.is_empty() || clause == "*" {
        return Ok(ComparatorSet {
            comparators: vec![],
        });
    }

    if let Some(idx) = find_hyphen(clause) {
        let lo = clause[..idx].trim();
        let hi = clause[idx + 3..].trim();
        return hyphen_range(lo, hi);
    }

    let glued = glue_operator_spaces(clause);
    let mut comparators = Vec::new();
    for tok in glued.split_whitespace() {
        comparators.extend(expand_comparator(tok)?);
    }
    Ok(ComparatorSet { comparators })
}

fn glue_operator_spaces(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if matches!(chars[i], '<' | '>' | '=' | '~' | '^') {

            while i < chars.len() && matches!(chars[i], '<' | '>' | '=' | '~' | '^') {
                out.push(chars[i]);
                i += 1;
            }
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn find_hyphen(s: &str) -> Option<usize> {
    s.find(" - ")
}

fn hyphen_range(lo: &str, hi: &str) -> Result<ComparatorSet, SemverError> {
    let (lp, lpre) = parse_partial(lo)?;
    let (hp, hpre) = parse_partial(hi)?;

    let low = Version {
        major: lp.major.unwrap_or(0),
        minor: lp.minor.unwrap_or(0),
        patch: lp.patch.unwrap_or(0),
        pre: lpre,
    };

    let high = if hp.major.is_none() {

        return Ok(ComparatorSet {
            comparators: vec![Comparator {
                op: Op::Gte,
                version: low,
            }],
        });
    } else if hp.minor.is_none() {
        Comparator {
            op: Op::Lt,
            version: Version::new(checked_inc(hp.major.unwrap(), "hyphen upper major")?, 0, 0),
        }
    } else if hp.patch.is_none() {
        Comparator {
            op: Op::Lt,
            version: Version::new(
                hp.major.unwrap(),
                checked_inc(hp.minor.unwrap(), "hyphen upper minor")?,
                0,
            ),
        }
    } else {
        Comparator {
            op: Op::Lte,
            version: Version {
                major: hp.major.unwrap(),
                minor: hp.minor.unwrap(),
                patch: hp.patch.unwrap(),
                pre: hpre,
            },
        }
    };
    Ok(ComparatorSet {
        comparators: vec![
            Comparator {
                op: Op::Gte,
                version: low,
            },
            high,
        ],
    })
}

fn expand_comparator(tok: &str) -> Result<Vec<Comparator>, SemverError> {
    let tok = tok.trim();
    if tok.is_empty() || tok == "*" || tok == "x" || tok == "X" {
        return Ok(vec![]);
    }

    if let Some(rest) = tok.strip_prefix('^') {
        return caret(rest);
    }
    if let Some(rest) = tok.strip_prefix('~') {
        return tilde(rest);
    }

    for (prefix, op) in [
        (">=", Op::Gte),
        ("<=", Op::Lte),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = tok.strip_prefix(prefix) {
            return operator_partial(op, rest);
        }
    }

    bare_partial(tok)
}

fn operator_partial(op: Op, rest: &str) -> Result<Vec<Comparator>, SemverError> {
    let (p, pre) = parse_partial(rest)?;
    let (x_maj, x_min, x_pat) = (p.major.is_none(), p.minor.is_none(), p.patch.is_none());
    let any_x = x_maj || x_min || x_pat;

    if matches!(op, Op::Eq) && any_x {
        return Ok(match (p.major, p.minor) {
            (None, _) => vec![],
            (Some(maj), None) => checked_band(maj, 0, 0, checked_inc(maj, "= major")?, 0, 0)?,
            (Some(maj), Some(min)) => {
                checked_band(maj, min, 0, maj, checked_inc(min, "= minor")?, 0)?
            }
        });
    }

    if x_maj {

        return Ok(match op {
            Op::Gt | Op::Lt => vec![Comparator {
                op: Op::Lt,
                version: Version::new(0, 0, 0),
            }],
            _ => vec![],
        });
    }

    let maj = p.major.unwrap();
    let min = p.minor.unwrap_or(0);
    let pat = p.patch.unwrap_or(0);

    if !any_x {

        return Ok(vec![Comparator {
            op,
            version: Version {
                major: maj,
                minor: min,
                patch: pat,
                pre,
            },
        }]);
    }

    let comp = match op {
        Op::Gt if x_min => Comparator {
            op: Op::Gte,
            version: Version::new(checked_inc(maj, "> major wildcard")?, 0, 0),
        },
        Op::Gt => Comparator {
            op: Op::Gte,
            version: Version::new(maj, checked_inc(min, "> minor wildcard")?, 0),
        },
        Op::Lte if x_min => Comparator {
            op: Op::Lt,
            version: Version::new(checked_inc(maj, "<= major wildcard")?, 0, 0),
        },
        Op::Lte => Comparator {
            op: Op::Lt,
            version: Version::new(maj, checked_inc(min, "<= minor wildcard")?, 0),
        },

        Op::Gte => Comparator {
            op: Op::Gte,
            version: Version::new(maj, min, 0),
        },
        Op::Lt => Comparator {
            op: Op::Lt,
            version: Version::new(maj, min, 0),
        },
        Op::Eq => unreachable!("= with wildcard handled above"),
    };
    Ok(vec![comp])
}

fn bare_partial(tok: &str) -> Result<Vec<Comparator>, SemverError> {
    let (p, pre) = parse_partial(tok)?;
    match (p.major, p.minor, p.patch) {
        (Some(maj), Some(min), Some(pat)) => {

            Ok(vec![Comparator {
                op: Op::Eq,
                version: Version {
                    major: maj,
                    minor: min,
                    patch: pat,
                    pre,
                },
            }])
        }
        (Some(maj), Some(min), None) => Ok(checked_band(
            maj,
            min,
            0,
            maj,
            checked_inc(min, "bare minor")?,
            0,
        )?),
        (Some(maj), None, _) => Ok(checked_band(
            maj,
            0,
            0,
            checked_inc(maj, "bare major")?,
            0,
            0,
        )?),
        (None, _, _) => Ok(vec![]),
    }
}

fn caret(rest: &str) -> Result<Vec<Comparator>, SemverError> {
    let (p, pre) = parse_partial(rest)?;
    let (maj, min, pat) = (p.major, p.minor, p.patch);
    let lo = Version {
        major: maj.unwrap_or(0),
        minor: min.unwrap_or(0),
        patch: pat.unwrap_or(0),
        pre,
    };
    let hi = match (maj, min, pat) {
        (Some(0), Some(0), Some(_)) => Version::new(0, 0, checked_inc(lo.patch, "caret patch")?),
        (Some(0), Some(0), None) => Version::new(0, 1, 0),
        (Some(0), Some(m), _) => Version::new(0, checked_inc(m, "caret minor")?, 0),
        (Some(0), None, _) => Version::new(1, 0, 0),
        (Some(maj), _, _) => Version::new(checked_inc(maj, "caret major")?, 0, 0),
        (None, _, _) => return Ok(vec![]),
    };
    Ok(vec![
        Comparator {
            op: Op::Gte,
            version: lo,
        },
        Comparator {
            op: Op::Lt,
            version: hi,
        },
    ])
}

fn tilde(rest: &str) -> Result<Vec<Comparator>, SemverError> {
    let (p, pre) = parse_partial(rest)?;
    match (p.major, p.minor, p.patch) {
        (Some(maj), Some(min), Some(pat)) => Ok(vec![
            Comparator {
                op: Op::Gte,
                version: Version {
                    major: maj,
                    minor: min,
                    patch: pat,
                    pre,
                },
            },
            Comparator {
                op: Op::Lt,
                version: Version::new(maj, checked_inc(min, "tilde full minor")?, 0),
            },
        ]),
        (Some(maj), Some(min), None) => Ok(checked_band(
            maj,
            min,
            0,
            maj,
            checked_inc(min, "tilde minor")?,
            0,
        )?),
        (Some(maj), None, _) => Ok(checked_band(
            maj,
            0,
            0,
            checked_inc(maj, "tilde major")?,
            0,
            0,
        )?),
        (None, _, _) => Ok(vec![]),
    }
}

fn band(lmaj: u64, lmin: u64, lpat: u64, hmaj: u64, hmin: u64, hpat: u64) -> Vec<Comparator> {
    vec![
        Comparator {
            op: Op::Gte,
            version: Version::new(lmaj, lmin, lpat),
        },
        Comparator {
            op: Op::Lt,
            version: Version::new(hmaj, hmin, hpat),
        },
    ]
}

fn checked_band(
    lmaj: u64,
    lmin: u64,
    lpat: u64,
    hmaj: u64,
    hmin: u64,
    hpat: u64,
) -> Result<Vec<Comparator>, SemverError> {
    Ok(band(lmaj, lmin, lpat, hmaj, hmin, hpat))
}

impl Op {

    fn canonical_str(self) -> &'static str {
        match self {
            Op::Lt => "<",
            Op::Lte => "<=",
            Op::Gt => ">",
            Op::Gte => ">=",
            Op::Eq => "=",
        }
    }
}

fn version_canonical(v: &Version) -> String {
    let mut s = format!("{}.{}.{}", v.major, v.minor, v.patch);
    if !v.pre.is_empty() {
        let pre: Vec<String> = v
            .pre
            .iter()
            .map(|id| match id {
                Identifier::Numeric(n) => n.to_string(),
                Identifier::AlphaNumeric(a) => a.clone(),
            })
            .collect();
        s.push('-');
        s.push_str(&pre.join("."));
    }
    s
}

impl ComparatorSet {

    pub fn canonical_string(&self) -> String {
        let mut parts: Vec<String> = self
            .comparators
            .iter()
            .map(|c| format!("{}{}", c.op.canonical_str(), version_canonical(&c.version)))
            .collect();
        parts.sort();
        parts.dedup();
        parts.join(" ")
    }
}

impl Range {

    pub fn canonical_string(&self) -> String {
        let mut sets: Vec<String> = self.sets.iter().map(|s| s.canonical_string()).collect();
        sets.sort();
        sets.dedup();
        sets.join(" || ")
    }
}

pub fn satisfies(range: &str, version: &str) -> Result<bool, SemverError> {
    let r = Range::parse(range)?;
    let v = Version::parse(version)?;
    Ok(r.satisfies(&v))
}

pub fn max_satisfying<'a>(range: &str, available: &'a [String]) -> Option<&'a str> {
    let r = Range::parse(range).ok()?;
    let mut best: Option<(&'a str, Version)> = None;
    for s in available {
        let v = match Version::parse(s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if r.satisfies(&v) {
            match &best {
                Some((_, bv)) if v.cmp_precedence(bv) != Ordering::Greater => {}
                _ => best = Some((s.as_str(), v)),
            }
        }
    }
    best.map(|(s, _)| s)
}

pub fn max_satisfying_all<'a>(ranges: &[String], available: &'a [String]) -> Option<&'a str> {
    let parsed: Vec<Range> = ranges
        .iter()
        .map(|r| Range::parse(r))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let mut best: Option<(&'a str, Version)> = None;
    for s in available {
        let v = match Version::parse(s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if parsed.iter().all(|r| r.satisfies(&v)) {
            match &best {
                Some((_, bv)) if v.cmp_precedence(bv) != Ordering::Greater => {}
                _ => best = Some((s.as_str(), v)),
            }
        }
    }
    best.map(|(s, _)| s)
}

pub fn version_satisfying_most<'a>(ranges: &[String], available: &'a [String]) -> Option<&'a str> {
    let parsed: Vec<Range> = ranges.iter().filter_map(|r| Range::parse(r).ok()).collect();
    let mut best: Option<(&'a str, usize, Version)> = None;
    for s in available {
        let v = match Version::parse(s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let count = parsed.iter().filter(|r| r.satisfies(&v)).count();
        if count == 0 {
            continue;
        }
        let better = match &best {
            None => true,
            Some((_, bc, bv)) => {
                count > *bc || (count == *bc && v.cmp_precedence(bv) == Ordering::Greater)
            }
        };
        if better {
            best = Some((s.as_str(), count, v));
        }
    }
    best.map(|(s, _, _)| s)
}

pub fn is_exact_pin(spec: &str) -> bool {
    let t = spec.trim();
    if t.starts_with(['^', '~', '>', '<', '=', 'v'])
        || t.contains('*')
        || t.contains(" - ")
        || t.contains("||")
        || t.contains(char::is_whitespace)
    {
        return false;
    }
    match parse_partial(t) {
        Ok((p, _)) => p.major.is_some() && p.minor.is_some() && p.patch.is_some(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_string_desugars_caret_and_tilde() {

        let caret = Range::parse("^1.2.3").unwrap().canonical_string();
        let explicit = Range::parse(">=1.2.3 <2.0.0").unwrap().canonical_string();
        let reordered = Range::parse("<2.0.0 >=1.2.3").unwrap().canonical_string();
        assert_eq!(
            caret, explicit,
            "^1.2.3 must canonicalize to >=1.2.3 <2.0.0"
        );
        assert_eq!(caret, reordered, "comparator order must normalize");

        assert_eq!(caret, "<2.0.0 >=1.2.3");
    }

    #[test]
    fn max_satisfying_all_compatible_diamond() {

        let avail: Vec<String> = ["1.0.0", "1.2.3", "1.2.9", "1.3.0", "2.0.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ranges = vec!["^1.0.0".to_string(), "~1.2.0".to_string()];
        assert_eq!(max_satisfying_all(&ranges, &avail), Some("1.2.9"));
    }

    #[test]
    fn max_satisfying_all_single_and_exact() {
        let avail: Vec<String> = ["1.0.0", "1.5.0", "2.0.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        assert_eq!(
            max_satisfying_all(&["^1.0.0".to_string()], &avail),
            Some("1.5.0")
        );

        assert_eq!(
            max_satisfying_all(&["^1.0.0".to_string(), "1.0.0".to_string()], &avail),
            Some("1.0.0")
        );
    }

    #[test]
    fn max_satisfying_all_incompatible_is_none() {

        let avail: Vec<String> = ["1.5.0", "2.3.0"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            max_satisfying_all(&["^1.0.0".to_string(), "^2.0.0".to_string()], &avail),
            None
        );
    }

    fn sat(r: &str, v: &str) -> bool {
        satisfies(r, v).unwrap_or(false)
    }

    #[test]
    fn version_ordering() {
        assert!(Version::parse("1.0.0").unwrap() < Version::parse("2.0.0").unwrap());
        assert!(Version::parse("1.0.0-alpha").unwrap() < Version::parse("1.0.0").unwrap());
        assert!(
            Version::parse("1.0.0-alpha.1").unwrap() < Version::parse("1.0.0-alpha.2").unwrap()
        );
        assert!(
            Version::parse("1.0.0-alpha.1").unwrap() < Version::parse("1.0.0-alpha.beta").unwrap()
        );
        assert!(Version::parse("1.0.0-rc.1").unwrap() < Version::parse("1.0.0").unwrap());

        assert_eq!(
            Version::parse("1.0.0+build")
                .unwrap()
                .cmp_precedence(&Version::parse("1.0.0").unwrap()),
            Ordering::Equal
        );
    }

    #[test]
    fn caret_semantics() {
        assert!(sat("^1.2.3", "1.2.3"));
        assert!(sat("^1.2.3", "1.9.9"));
        assert!(!sat("^1.2.3", "2.0.0"));
        assert!(!sat("^1.2.3", "1.2.2"));
        assert!(sat("^0.2.3", "0.2.9"));
        assert!(!sat("^0.2.3", "0.3.0"));
        assert!(sat("^0.0.3", "0.0.3"));
        assert!(!sat("^0.0.3", "0.0.4"));
        assert!(sat("^1.x", "1.5.0"));
        assert!(!sat("^1.x", "2.0.0"));
    }

    #[test]
    fn tilde_semantics() {
        assert!(sat("~1.2.3", "1.2.9"));
        assert!(!sat("~1.2.3", "1.3.0"));
        assert!(sat("~1.2", "1.2.0"));
        assert!(!sat("~1.2", "1.3.0"));
        assert!(sat("~1", "1.9.9"));
        assert!(!sat("~1", "2.0.0"));
    }

    #[test]
    fn comparators_and_ranges() {
        assert!(sat(">=1.2.3 <2.0.0", "1.5.0"));
        assert!(!sat(">=1.2.3 <2.0.0", "2.0.0"));
        assert!(sat("1.2.3 - 2.3.4", "2.0.0"));
        assert!(!sat("1.2.3 - 2.3.4", "2.3.5"));
        assert!(sat("1.2 - 2.3", "2.3.9"));
        assert!(!sat("1.2 - 2.3", "2.4.0"));
        assert!(sat("^1.0.0 || ^2.0.0", "2.5.0"));
        assert!(!sat("^1.0.0 || ^2.0.0", "3.0.0"));
        assert!(sat("*", "9.9.9"));
        assert!(sat("1.x", "1.9.9"));
        assert!(!sat("1.x", "2.0.0"));
    }

    #[test]
    fn bare_is_exact_node_semver() {

        assert!(sat("1.2.3", "1.2.3"));
        assert!(!sat("1.2.3", "1.2.4"));
        assert!(!sat("1.2.3", "1.3.0"));
        assert!(is_exact_pin("1.2.3"));
        assert!(!is_exact_pin("^1.2.3"));
        assert!(!is_exact_pin("1.2"));
        assert!(!is_exact_pin("1.x"));
    }

    #[test]
    fn prerelease_gating() {

        assert!(sat(">=1.0.0-alpha", "1.0.0-alpha.1"));
        assert!(sat(">=1.0.0-alpha <1.0.0", "1.0.0-beta"));

        assert!(!sat(">=1.0.0", "1.1.0-alpha"));
        assert!(!sat("^1.0.0", "2.0.0-alpha"));
        assert!(!sat("*", "1.0.0-alpha"));

        assert!(sat("1.0.0-rc.1", "1.0.0-rc.1"));
        assert!(!sat("1.0.0-rc.1", "1.0.0-rc.2"));
    }

    #[test]
    fn hostile_prerelease_numeric_overflow_returns_error() {
        let huge = "1.0.0-99999999999999999999999999";
        assert!(Version::parse(huge).is_err());
        assert!(Range::parse(huge).is_err());
    }

    #[test]
    fn hostile_range_component_overflow_returns_error() {
        let max = u64::MAX;
        for range in [
            format!("{max}"),
            format!("={max}"),
            format!(">{max}"),
            format!("<={max}"),
            format!("^{max}.0.0"),
            format!("~{max}"),
            format!("1.0.0 - {max}"),
            format!("1.0.0 - 1.{max}"),
        ] {
            assert!(Range::parse(&range).is_err(), "{range} should fail closed");
        }
    }

    #[test]
    fn max_satisfying_selection() {
        let avail: Vec<String> = ["1.0.0", "1.2.0", "1.2.3", "1.9.0", "2.0.0", "2.1.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(max_satisfying("^1.2.0", &avail), Some("1.9.0"));
        assert_eq!(max_satisfying("~1.2.0", &avail), Some("1.2.3"));
        assert_eq!(max_satisfying(">=1.0.0 <2.0.0", &avail), Some("1.9.0"));
        assert_eq!(max_satisfying("2.x", &avail), Some("2.1.0"));
        assert_eq!(max_satisfying("3.x", &avail), None);
    }

    #[test]
    fn partial_version_with_operator_xrange_semantics() {

        assert!(satisfies(">1", "2.0.0").unwrap());
        assert!(!satisfies(">1", "1.9.9").unwrap());
        assert!(satisfies(">2", "3.0.0").unwrap());
        assert!(!satisfies(">2", "2.9.9").unwrap());
        assert!(satisfies(">1.2", "1.3.0").unwrap());
        assert!(!satisfies(">1.2", "1.2.9").unwrap());
        assert!(satisfies(">=1", "1.0.0").unwrap());
        assert!(!satisfies(">=1", "0.9.9").unwrap());
        assert!(satisfies("<1", "0.9.9").unwrap());
        assert!(!satisfies("<1", "1.0.0").unwrap());
        assert!(satisfies("<=1", "1.9.9").unwrap());
        assert!(!satisfies("<=1", "2.0.0").unwrap());
        assert!(satisfies("<=1.2", "1.2.9").unwrap());
        assert!(!satisfies("<=1.2", "1.3.0").unwrap());
        assert!(satisfies("=1.2", "1.2.5").unwrap());
        assert!(!satisfies("=1.2", "1.3.0").unwrap());
        assert!(satisfies("=1", "1.5.0").unwrap());
        assert!(!satisfies("=1", "2.0.0").unwrap());
    }

    #[test]
    fn operator_space_before_version() {

        assert!(satisfies(">= 2.1.2 < 3", "2.1.2").unwrap());
        assert!(satisfies(">= 2.1.2 < 3", "2.9.9").unwrap());
        assert!(!satisfies(">= 2.1.2 < 3", "2.1.1").unwrap());
        assert!(!satisfies(">= 2.1.2 < 3", "3.0.0").unwrap());
        assert!(satisfies("> 1.0.0", "1.0.1").unwrap());
        assert!(satisfies("<= 2.0.0", "2.0.0").unwrap());
        let avail: Vec<String> = ["2.1.1", "2.1.2", "2.9.9", "3.0.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(max_satisfying(">= 2.1.2 < 3", &avail), Some("2.9.9"));
    }

    #[test]
    fn cross_validate_agreement_subset_vs_cargo_semver() {
        use semver::{Version as CV, VersionReq};
        let ranges = [">=1.2.0, <2.0.0", "^1.2.3", "~1.2.3", "^0.2.3", "1.2.*"];

        let node_equiv = [">=1.2.0 <2.0.0", "^1.2.3", "~1.2.3", "^0.2.3", "1.2.x"];
        let versions = [
            "1.1.0", "1.2.0", "1.2.3", "1.5.0", "1.9.9", "2.0.0", "0.2.3", "0.2.9", "0.3.0",
        ];
        for (cargo_r, node_r) in ranges.iter().zip(node_equiv.iter()) {
            let req = VersionReq::parse(cargo_r).unwrap();
            let our = Range::parse(node_r).unwrap();
            for vs in versions.iter() {
                let cv = CV::parse(vs).unwrap();
                let ours = our.satisfies(&Version::parse(vs).unwrap());
                let theirs = req.matches(&cv);
                assert_eq!(
                    ours, theirs,
                    "divergence on range `{node_r}` (cargo `{cargo_r}`) version `{vs}`: ours={ours} cargo={theirs}"
                );
            }
        }
    }
}
