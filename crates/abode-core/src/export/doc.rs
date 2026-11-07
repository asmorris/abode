//! A small, format-neutral description of a document (headings,
//! paragraphs, tables, images), so the Markdown and PDF exports are built
//! from the same content.

/// A run of text inside a paragraph.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    /// `href` is relative to the export folder (e.g. `pages/roof.md`).
    Link { text: String, href: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Level 1–4.
    Heading(u8, String),
    Para(Vec<Inline>),
    /// Text with its line breaks kept (note bodies).
    Lines(String),
    /// "Label: value" pairs.
    Fields(Vec<(String, String)>),
    Table { head: Vec<String>, rows: Vec<Vec<String>> },
    Bullets(Vec<Vec<Inline>>),
    /// `src` is relative to the export folder.
    Image { src: String, alt: String },
}

pub fn text(s: impl Into<String>) -> Inline {
    Inline::Text(s.into())
}

pub fn link(t: impl Into<String>, href: impl Into<String>) -> Inline {
    Inline::Link { text: t.into(), href: href.into() }
}

pub fn para(s: impl Into<String>) -> Block {
    Block::Para(vec![text(s)])
}

/// One page of the export.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// Where it's written, relative to the export folder (`README.md`, `pages/roof.md`).
    pub path: String,
    pub title: String,
    pub blocks: Vec<Block>,
}

impl Page {
    pub fn new(path: impl Into<String>, title: impl Into<String>) -> Self {
        let title = title.into();
        Self { path: path.into(), blocks: vec![Block::Heading(1, title.clone())], title }
    }

    pub fn push(&mut self, b: Block) {
        self.blocks.push(b);
    }

    pub fn extend(&mut self, bs: impl IntoIterator<Item = Block>) {
        self.blocks.extend(bs);
    }
}

/// Fields with empty values dropped.
pub fn fields<'a>(pairs: impl IntoIterator<Item = (&'a str, String)>) -> Option<Block> {
    let v: Vec<(String, String)> = pairs
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.to_string(), v.trim().to_string()))
        .collect();
    (!v.is_empty()).then_some(Block::Fields(v))
}

// ---------------------------------------------------------------------------
// Markdown

/// `href` (relative to the export folder) as seen from the page at `from`.
pub fn relative(from: &str, href: &str) -> String {
    let depth = from.matches('/').count();
    let from_dir: Vec<&str> = from.split('/').take(depth).collect();
    let to: Vec<&str> = href.split('/').collect();
    let common = from_dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from_dir.len() - common];
    parts.extend(&to[common..]);
    parts.join("/")
}

/// A link target that survives spaces and brackets in file names.
fn url(path: &str) -> String {
    path.replace('%', "%25").replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

/// Escape characters Markdown would otherwise treat as formatting, while
/// keeping the text easy to read in a plain text editor.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for ch in s.chars() {
        let escape_it = match ch {
            '\\' | '*' | '`' | '[' | ']' | '<' | '>' => true,
            // These only matter at the start of a line.
            '#' | '-' | '+' | '=' => prev.is_none(),
            // snake_case is fine; _emphasis_ isn't.
            '_' => prev.is_none_or(char::is_whitespace),
            _ => false,
        };
        if escape_it {
            out.push('\\');
        }
        out.push(ch);
        prev = Some(ch);
    }
    // "1. Something" at the start of a line would become a list.
    if let Some(dot) = out.find(". ") {
        if dot > 0 && out[..dot].chars().all(|c| c.is_ascii_digit()) {
            out.insert(dot, '\\');
        }
    }
    out
}

fn cell(s: &str) -> String {
    escape(s.trim()).replace('|', "\\|").replace("\r\n", "\n").replace('\n', "<br>")
}

fn inline(page: &str, i: &Inline) -> String {
    match i {
        Inline::Text(t) => escape(t),
        Inline::Bold(t) => format!("**{}**", escape(t)),
        Inline::Italic(t) => format!("*{}*", escape(t)),
        Inline::Link { text, href } => format!("[{}]({})", escape(text), url(&relative(page, href))),
    }
}

pub fn to_markdown(page: &Page) -> String {
    let mut out = String::new();
    for b in &page.blocks {
        match b {
            Block::Heading(level, t) => {
                out.push_str(&"#".repeat((*level).clamp(1, 6) as usize));
                out.push(' ');
                out.push_str(&escape(t));
                out.push('\n');
            }
            Block::Para(parts) => {
                for p in parts {
                    out.push_str(&inline(&page.path, p));
                }
                out.push('\n');
            }
            Block::Lines(t) => {
                let lines: Vec<String> = t.trim().lines().map(|l| escape(l.trim_end())).collect();
                // A trailing backslash is a line break that keeps paragraphs intact.
                out.push_str(&lines.join("\\\n").replace("\\\n\\\n", "\n\n"));
                out.push('\n');
            }
            Block::Fields(pairs) => {
                for (k, v) in pairs {
                    let v: Vec<String> = v.trim().lines().map(|l| escape(l.trim_end())).collect();
                    out.push_str(&format!("- **{}:** {}\n", escape(k), v.join("\\\n  ")));
                }
            }
            Block::Table { head, rows } => {
                out.push_str(&format!("| {} |\n", head.iter().map(|h| cell(h)).collect::<Vec<_>>().join(" | ")));
                out.push_str(&format!("|{}\n", " --- |".repeat(head.len())));
                for r in rows {
                    let mut cells: Vec<String> = r.iter().map(|c| cell(c)).collect();
                    cells.resize(head.len(), String::new());
                    out.push_str(&format!("| {} |\n", cells.join(" | ")));
                }
            }
            Block::Bullets(items) => {
                for it in items {
                    out.push_str("- ");
                    for p in it {
                        out.push_str(&inline(&page.path, p));
                    }
                    out.push('\n');
                }
            }
            Block::Image { src, alt } => {
                out.push_str(&format!("![{}]({})\n", escape(alt), url(&relative(&page.path, src))));
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_links() {
        assert_eq!(relative("README.md", "pages/roof.md"), "pages/roof.md");
        assert_eq!(relative("pages/roof.md", "pages/paint.md"), "paint.md");
        assert_eq!(relative("pages/roof.md", "plans/roof.svg"), "../plans/roof.svg");
        assert_eq!(relative("pages/roof.md", "README.md"), "../README.md");
    }

    #[test]
    fn escapes_only_what_matters() {
        assert_eq!(escape("Pressure-treated pine"), "Pressure-treated pine");
        assert_eq!(escape("- not a list"), "\\- not a list");
        assert_eq!(escape("2. not a list"), "2\\. not a list");
        assert_eq!(escape("a*b [c]"), "a\\*b \\[c\\]");
        assert_eq!(escape("snake_case"), "snake_case");
    }

    #[test]
    fn renders_a_page() {
        let mut p = Page::new("pages/x.md", "Kitchen");
        p.push(Block::Fields(vec![("Area".into(), "16 m²".into())]));
        p.push(Block::Table { head: vec!["A".into(), "B".into()], rows: vec![vec!["1|2".into(), "x\ny".into()]] });
        p.push(Block::Image { src: "plans/main floor.svg".into(), alt: "Plan".into() });
        let md = to_markdown(&p);
        assert!(md.starts_with("# Kitchen\n"));
        assert!(md.contains("- **Area:** 16 m²"));
        assert!(md.contains("| 1\\|2 | x<br>y |"));
        assert!(md.contains("![Plan](../plans/main%20floor.svg)"));
    }
}
