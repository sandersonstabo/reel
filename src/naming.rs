#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

const MAX_NAME: usize = 60;

pub fn sanitize(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(MAX_NAME).collect();
    truncated.trim_end_matches(['.', ' ']).to_string()
}

pub fn display_name(title: &str, exe_stem: &str) -> String {
    let from_title = sanitize(title);
    match from_title.chars().any(char::is_alphanumeric) {
        true => from_title,
        false => match sanitize(exe_stem) {
            stem if stem.is_empty() => "Game".to_string(),
            stem => stem,
        },
    }
}

pub fn file_name(name: &str, stamp: Stamp) -> String {
    format!(
        "{name} {:04}-{:02}-{:02} {:02}-{:02}-{:02}.mp4",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    )
}
