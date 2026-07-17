/// Filing rules: match a filename against a pattern.
///
/// `ext`  — comma-separated extensions, e.g. "gbr,drl,gtl"
/// `glob` — comma-separated globs, e.g. "BOM*.csv,*.step"
/// `regex`— a raw regex (applied case-insensitively)
pub fn rule_matches(match_kind: &str, pattern: &str, file_name: &str) -> bool {
    let name = file_name.to_lowercase();
    match match_kind {
        "ext" => {
            let ext = name.rsplit('.').next().unwrap_or("");
            pattern
                .split(',')
                .map(|p| p.trim().trim_start_matches('.').to_lowercase())
                .any(|p| !p.is_empty() && p == ext)
        }
        "glob" => pattern
            .split(',')
            .map(|p| p.trim().to_lowercase())
            .any(|p| !p.is_empty() && glob_match(&p, &name)),
        "regex" => regex_lite_match(pattern, &name),
        _ => false,
    }
}

/// Minimal glob: `*` matches any run, `?` matches one char.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut star_ti) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Tiny regex support without pulling in the regex crate: we only honor a
/// practical subset (`^`, `$`, `.`, `.*`, literal text). Falls back to
/// substring match when the pattern uses anything fancier.
fn regex_lite_match(pattern: &str, text: &str) -> bool {
    let pat = pattern.to_lowercase();
    let anchored_start = pat.starts_with('^');
    let anchored_end = pat.ends_with('$');
    let core = pat.trim_start_matches('^').trim_end_matches('$');
    // Translate the subset into a glob: `.*` → `*`, `.` → `?`.
    let glob: String = {
        let mut out = String::new();
        let mut chars = core.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '.' {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    out.push('*');
                } else {
                    out.push('?');
                }
            } else if "\\+[](){}|$^".contains(c) {
                // Unsupported syntax → substring fallback.
                return text.contains(core.trim_matches(|ch: char| !ch.is_alphanumeric()));
            } else {
                out.push(c);
            }
        }
        out
    };
    let glob = match (anchored_start, anchored_end) {
        (true, true) => glob,
        (true, false) => format!("{glob}*"),
        (false, true) => format!("*{glob}"),
        (false, false) => format!("*{glob}*"),
    };
    glob_match(&glob, text)
}

/// Built-in starter rules for new setups: (pattern, match kind, bin name).
pub const DEFAULT_RULES: &[(&str, &str, &str)] = &[
    ("gbr,gtl,gbl,gto,gbo,gts,gbs,gko,gm1,drl,xln", "ext", "Gerbers"),
    ("step,stp,stl,f3d,dxf,3mf", "ext", "CAD"),
    ("BOM*.csv,BOM*.xlsx,*bom*.csv", "glob", "BOM"),
    ("pdf", "ext", "Datasheets"),
    ("png,jpg,jpeg,heic,gif,mp4,mov", "ext", "Photos"),
    ("ino,c,cpp,h,hpp,bin,elf,hex", "ext", "Firmware"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_rules() {
        assert!(rule_matches("ext", "gbr,drl", "board-F_Cu.GBR"));
        assert!(rule_matches("ext", ".step", "enclosure.STEP"));
        assert!(!rule_matches("ext", "gbr", "readme.md"));
    }

    #[test]
    fn glob_rules() {
        assert!(rule_matches("glob", "BOM*.csv", "BOM_rev2.csv"));
        assert!(rule_matches("glob", "*.step,*.stl", "case.stl"));
        assert!(!rule_matches("glob", "BOM*.csv", "notes.csv"));
    }

    #[test]
    fn regex_rules() {
        assert!(rule_matches("regex", "^verdant.*zip$", "verdant-gerbers.zip"));
        assert!(rule_matches("regex", "^bom.*csv$", "bom_rev1.csv"));
        assert!(rule_matches("regex", "gerber", "my-gerber-files.zip"));
    }
}
