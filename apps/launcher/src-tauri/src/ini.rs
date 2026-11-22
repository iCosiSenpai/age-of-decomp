//! Line-preserving editor for the runtime's `hwaoi.ini`, compatible with how the runtime itself rewrites it
//! (runtime/cfg.c): keys match case-insensitively with an optional `TOTK_`/`HWAOI_`/`DKB_` prefix, the last line of a
//! key wins, a replaced line keeps its key spelling and becomes `key = value`, new keys are written in lower case
//! into their section (created at the end when missing). Comments, order and unknown keys are kept.

use std::collections::BTreeMap;
use std::path::Path;

pub struct Ini {
    lines: Vec<String>,
    crlf: bool,
}

fn norm(key: &str) -> String {
    let k = key.trim().to_ascii_uppercase();
    for p in ["TOTK_", "HWAOI_", "DKB_"] {
        if let Some(rest) = k.strip_prefix(p) {
            return rest.to_string();
        }
    }
    k
}

/// The normalised key of a `key = value` line, or None.
fn line_key(line: &str) -> Option<String> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with(';') || t.starts_with('[') {
        return None;
    }
    let (k, _) = t.split_once('=')?;
    let k = k.trim_end();
    if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(norm(k))
}

fn line_value(line: &str) -> String {
    let v = line.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
    let v = v.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(v);
    v.to_string()
}

fn line_section(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.split_once(']')?.0;
    Some(inner.trim().to_ascii_lowercase())
}

impl Ini {
    pub fn load(path: &Path) -> std::io::Result<Ini> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{} is not UTF-8 text", path.display())))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Ok(Ini { crlf: text.contains("\r\n"), lines: text.lines().map(str::to_string).collect() })
    }

    /// Every key set in the file, normalised to lower case, last line wins; empty values are left out (they mean
    /// "default" to the runtime).
    pub fn values(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for l in &self.lines {
            if let Some(k) = line_key(l) {
                let v = line_value(l);
                if v.is_empty() {
                    out.remove(&k.to_ascii_lowercase());
                } else {
                    out.insert(k.to_ascii_lowercase(), v);
                }
            }
        }
        out
    }

    pub fn set(&mut self, key: &str, value: &str, section: &str) {
        let want = norm(key);
        if let Some(i) = self.lines.iter().rposition(|l| line_key(l).as_deref() == Some(&want)) {
            let l = &self.lines[i];
            let indent: String = l.chars().take_while(|c| c.is_whitespace()).collect();
            let spelled = l.trim_start().split_once('=').map(|(k, _)| k.trim_end().to_string()).unwrap_or_default();
            self.lines[i] = format!("{indent}{spelled} = {value}");
            return;
        }
        let entry = format!("{} = {}", key.to_ascii_lowercase(), value);
        let section = section.to_ascii_lowercase();
        let mut in_sec = false;
        let mut tail = None;
        for (i, l) in self.lines.iter().enumerate() {
            if let Some(s) = line_section(l) {
                in_sec = s == section;
                if in_sec {
                    tail = Some(i);
                }
                continue;
            }
            if in_sec && !l.trim().is_empty() {
                tail = Some(i);
            }
        }
        match tail {
            Some(i) => self.lines.insert(i + 1, entry),
            None => {
                if self.lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    self.lines.push(String::new());
                }
                if !section.is_empty() {
                    self.lines.push(format!("[{section}]"));
                }
                self.lines.push(entry);
            }
        }
    }

    /// Removes every line of `key`: the runtime then uses its default.
    pub fn remove(&mut self, key: &str) {
        let want = norm(key);
        self.lines.retain(|l| line_key(l).as_deref() != Some(&want));
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let eol = if self.crlf { "\r\n" } else { "\n" };
        let mut text = self.lines.join(eol);
        text.push_str(eol);
        // A name of this process's own: the launcher's window, aod-cli and RecompLauncher can each write this file,
        // and none must rename what another is still writing.
        let tmp = path.with_extension(format!("ini.{}.part", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> Ini {
        Ini { crlf: false, lines: text.lines().map(str::to_string).collect() }
    }

    #[test]
    fn reads_prefixed_and_last_wins() {
        let i = ini("[settings]\noutput_res=1920x1080\nTOTK_FPS_LIMIT = 60\nfps_limit = 30\n# volume = 50\n");
        let v = i.values();
        assert_eq!(v.get("output_res").map(String::as_str), Some("1920x1080"));
        assert_eq!(v.get("fps_limit").map(String::as_str), Some("30"));
        assert!(!v.contains_key("volume"));
    }

    #[test]
    fn replaces_in_place_and_appends_into_section() {
        let mut i = ini("[settings]\noutput_res=1920x1080\nrender_scale=100\n\n[audio]\n# comment\n");
        i.set("OUTPUT_RES", "1280x720", "display");
        i.set("volume", "150", "audio");
        i.set("motion", "0", "input");
        let t = i.lines.join("\n");
        assert!(t.contains("output_res = 1280x720"));
        assert!(t.contains("[audio]\n# comment\nvolume = 150"));
        assert!(t.ends_with("[input]\nmotion = 0"));
    }

    #[test]
    fn remove_resets_to_default() {
        let mut i = ini("fps_limit = 90\nFPS_LIMIT=120\nvolume=10\n");
        i.remove("fps_limit");
        assert_eq!(i.values().len(), 1);
    }
}
