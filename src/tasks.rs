const DEFAULT: &str = include_str!("../assets/default_tasks.txt");

pub fn tasks_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .map(|p| p.with_file_name("tasks.txt"))
}

pub fn load() -> Vec<String> {
    let text = tasks_path().and_then(|p| std::fs::read_to_string(p).ok());
    parse(text.as_deref().unwrap_or(DEFAULT))
}

pub fn parse(s: &str) -> Vec<String> {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn skips_blank_lines() {
        assert_eq!(parse("a\n\n  \nb"), ["a", "b"]);
    }

    #[test]
    fn skips_comments() {
        assert_eq!(parse("# c\na\n  # d\nb"), ["a", "b"]);
    }

    #[test]
    fn strips_bom() {
        assert_eq!(parse("\u{feff}# c\na"), ["a"]);
        assert_eq!(parse("\u{feff}a"), ["a"]);
    }

    #[test]
    fn empty_file() {
        assert!(parse("").is_empty());
        assert!(parse("\u{feff}").is_empty());
    }
}
