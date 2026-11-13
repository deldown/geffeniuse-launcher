//! Turns a crashed game's output into a plain explanation. Only well-known patterns are named;
//! anything else gets no hint, so the launcher never guesses.
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    /// Stable key the UI can translate.
    pub kind: &'static str,
    /// Mod or file the message names, if it names one.
    pub subject: Option<String>,
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.split_once(marker).map(|(_, r)| r)
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`' || c == ':' || c == '.' || c == ',');
    let s: String = s.chars().take(80).collect();
    (!s.is_empty()).then_some(s)
}

pub fn diagnose(lines: &[String], exit_code: Option<i32>) -> Option<Hint> {
    let text = |needle: &str| lines.iter().any(|l| l.contains(needle));
    let hint = |kind, subject| Some(Hint { kind, subject });
    if text("UnsupportedClassVersionError") || text("has been compiled by a more recent version of the Java Runtime") {
        return hint("java_too_old", None);
    }
    if text("jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to class java.net.URLClassLoader") {
        return hint("java_too_new", None);
    }
    if text("java.lang.OutOfMemoryError") || text("Java heap space") || text("GC overhead limit exceeded") {
        return hint("out_of_memory", None);
    }
    if let Some(l) = lines.iter().find(|l| l.contains("Duplicate mods") || l.contains("Found duplicate mods")) {
        return hint("duplicate_mods", clean(after(l, "mods").unwrap_or("")));
    }
    for l in lines {
        for marker in ["Could not find required mod:", "requires mod ", "requires any version of ", "Missing or unsupported mandatory dependencies"] {
            if l.contains(marker) {
                let subject = after(l, marker).and_then(|r| clean(r.split_whitespace().next().unwrap_or("")));
                return hint("missing_dependency", subject);
            }
        }
        if l.contains("Mod ") && l.contains(" requires ") && l.contains(" of ") {
            return hint("missing_dependency", l.split(" requires ").nth(1).and_then(|r| r.split(" of ").nth(1)).and_then(|r| clean(r.split_whitespace().next().unwrap_or(""))));
        }
    }
    if text("Incompatible mods found") || text("Incompatible mod set") {
        return hint("incompatible_mods", None);
    }
    if let Some(l) = lines.iter().find(|l| l.contains("Mixin apply failed") || l.contains("InvalidMixinException") || l.contains("MixinApplyError")) {
        let subject = after(l, "mixins.json").or_else(|| after(l, "Mixin config ")).and_then(|r| clean(r.split_whitespace().next().unwrap_or("")));
        return hint("mod_conflict", subject);
    }
    if text("GLFW error") || text("Failed to create window") || text("Pixel format not accelerated") || text("OpenGL") && text("not supported") {
        return hint("graphics", None);
    }
    if exit_code == Some(137) || exit_code == Some(-1073741819) {
        return hint("killed", None);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    #[test]
    fn names_the_wrong_java() {
        assert_eq!(diagnose(&l("java.lang.UnsupportedClassVersionError: x has been compiled"), Some(1)).unwrap().kind, "java_too_old");
        let forge = "Exception in thread \"main\" java.lang.ClassCastException: class jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to class java.net.URLClassLoader";
        assert_eq!(diagnose(&l(forge), Some(1)).unwrap().kind, "java_too_new");
    }

    #[test]
    fn names_memory_and_graphics() {
        assert_eq!(diagnose(&l("Caused by: java.lang.OutOfMemoryError: Java heap space"), Some(1)).unwrap().kind, "out_of_memory");
        assert_eq!(diagnose(&l("GLFW error 65542: WGL: The driver does not appear to support OpenGL"), Some(1)).unwrap().kind, "graphics");
        assert_eq!(diagnose(&[], Some(137)).unwrap().kind, "killed");
    }

    #[test]
    fn names_the_missing_mod() {
        let fabric = l("Could not find required mod: fabric-api\nmore");
        assert_eq!(diagnose(&fabric, Some(1)), Some(Hint { kind: "missing_dependency", subject: Some("fabric-api".into()) }));
        let forge = l("Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'x'");
        assert_eq!(diagnose(&forge, Some(1)).unwrap().kind, "missing_dependency");
    }

    #[test]
    fn unknown_crashes_get_no_hint() {
        assert_eq!(diagnose(&l("[main/INFO]: Stopping!"), Some(1)), None);
        assert_eq!(diagnose(&[], Some(1)), None);
    }
}
