//! The house manual as a PDF, laid out from the same pages as the Markdown
//! export: each page starts a chapter, with bookmarks, page numbers and a
//! contents list that links to them. Plans are drawn as vector graphics.

use super::doc::{Block, Inline, Page};
use super::svg::{Cap, Drawing, Rgb, Shape};
use krilla::Document;
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::color::rgb;
use krilla::destination::XyzDestination;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::outline::{Outline, OutlineNode};
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, LineCap, LineJoin, Stroke, StrokeDash};
use krilla::surface::Surface;
use krilla::text::{Font, TextDirection};
use std::collections::BTreeMap;

// The app's own fonts (they're small: Latin only).
const INTER: &[u8] = include_bytes!("../../../abode-app/assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../../abode-app/assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../abode-app/assets/fonts/Inter-SemiBold.ttf");
const SERIF: &[u8] = include_bytes!("../../../abode-app/assets/fonts/InstrumentSerif-Regular.ttf");

const INK: Rgb = [34, 38, 36];
const MUTED: Rgb = [112, 118, 115];
const ACCENT: Rgb = [28, 117, 94];
const RULE: Rgb = [218, 221, 218];
const SHADE: Rgb = [243, 244, 242];

const BODY: f32 = 10.0;
const SMALL: f32 = 8.5;
const LEADING: f32 = 1.42;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum F {
    Regular,
    Medium,
    Bold,
    Serif,
}

struct Fonts {
    faces: [ttf_parser::Face<'static>; 4],
    pdf: [Font; 4],
}

impl Fonts {
    fn load() -> Result<Self, String> {
        let data = [INTER, INTER_MEDIUM, INTER_SEMIBOLD, SERIF];
        let face = |d: &'static [u8]| ttf_parser::Face::parse(d, 0).map_err(|e| format!("font: {e}"));
        let pdf = |d: &'static [u8]| Font::new(d.to_vec().into(), 0).ok_or_else(|| "font couldn't be loaded".to_string());
        Ok(Self {
            faces: [face(data[0])?, face(data[1])?, face(data[2])?, face(data[3])?],
            pdf: [pdf(data[0])?, pdf(data[1])?, pdf(data[2])?, pdf(data[3])?],
        })
    }

    fn i(f: F) -> usize {
        match f {
            F::Regular => 0,
            F::Medium => 1,
            F::Bold => 2,
            F::Serif => 3,
        }
    }

    /// Width of `s` at `size` (no kerning, so a touch generous).
    fn width(&self, f: F, size: f32, s: &str) -> f32 {
        let face = &self.faces[Self::i(f)];
        let upm = face.units_per_em() as f32;
        let units: u32 = s.chars().map(|c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(upm as u16 / 2) as u32).sum();
        units as f32 / upm * size
    }

    /// Replace characters the fonts don't have with ones they do.
    fn clean(&self, s: &str) -> String {
        let face = &self.faces[0];
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            if c == '\t' {
                out.push_str("    ");
            } else if c.is_control() {
                continue;
            } else if face.glyph_index(c).is_some() {
                out.push(c);
            } else {
                out.push_str(match c {
                    '…' | '⋯' => "...",
                    '→' => "->",
                    '←' => "<-",
                    '•' | '·' => "-",
                    '‘' | '’' => "'",
                    '“' | '”' => "\"",
                    '–' | '—' => "-",
                    '×' => "x",
                    '²' => "2",
                    '³' => "3",
                    '°' => " deg",
                    _ => "?",
                });
            }
        }
        out
    }
}

/// Things to draw on a page, in points from the top-left corner.
enum Op {
    Text { x: f32, y: f32, f: F, size: f32, color: Rgb, text: String },
    Rect { x: f32, y: f32, w: f32, h: f32, fill: Rgb },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32 },
    Drawing { x: f32, y: f32, scale: f32, drawing: usize },
    Image { x: f32, y: f32, w: f32, h: f32, image: usize },
    /// A link to the start of another page of the export.
    Link { x: f32, y: f32, w: f32, h: f32, to: String },
    /// The page number where `to` starts, right-aligned at `x`.
    PageNumber { x: f32, y: f32, to: String },
}

/// A run of text with one style.
#[derive(Clone)]
struct Span {
    text: String,
    f: F,
    size: f32,
    color: Rgb,
    link: Option<String>,
}

/// A bookmark: its title and where it points (page index, y).
type Mark = (String, usize, f32);

/// A piece of a wrapped line: where it goes and what it says.
struct Placed {
    x: f32,
    w: f32,
    span: usize,
    text: String,
}

pub struct Settings<'a> {
    pub title: &'a str,
    /// US Letter instead of A4.
    pub letter: bool,
}

/// Lay out the pages and write the PDF.
pub fn render(pages: &[Page], drawings: &BTreeMap<String, Drawing>, images: &BTreeMap<String, Vec<u8>>, settings: Settings) -> Result<Vec<u8>, String> {
    let fonts = Fonts::load()?;
    let size = if settings.letter { (612.0, 792.0) } else { (595.28, 841.89) };
    let mut l = Layout::new(&fonts, size, drawings, images);
    for (i, page) in pages.iter().enumerate() {
        l.chapter(page, i == 0);
    }
    l.finish(settings.title)
}

struct Layout<'a> {
    fonts: &'a Fonts,
    size: (f32, f32),
    pages: Vec<Vec<Op>>,
    y: f32,
    /// Page path → (page index, y) where it starts.
    starts: BTreeMap<String, (usize, f32)>,
    /// Bookmarks: chapter title and where, with its sections.
    outline: Vec<(Mark, Vec<Mark>)>,
    drawings: &'a BTreeMap<String, Drawing>,
    drawing_list: Vec<&'a Drawing>,
    images: &'a BTreeMap<String, Vec<u8>>,
    image_list: Vec<Image>,
    /// The page being laid out (for links between pages).
    current: String,
}

const MARGIN_X: f32 = 56.0;
const MARGIN_TOP: f32 = 54.0;
const MARGIN_BOTTOM: f32 = 62.0;

impl<'a> Layout<'a> {
    fn new(fonts: &'a Fonts, size: (f32, f32), drawings: &'a BTreeMap<String, Drawing>, images: &'a BTreeMap<String, Vec<u8>>) -> Self {
        Self {
            fonts,
            size,
            pages: Vec::new(),
            y: MARGIN_TOP,
            starts: BTreeMap::new(),
            outline: Vec::new(),
            drawings,
            drawing_list: Vec::new(),
            images,
            image_list: Vec::new(),
            current: String::new(),
        }
    }

    fn left(&self) -> f32 {
        MARGIN_X
    }

    fn width(&self) -> f32 {
        self.size.0 - 2.0 * MARGIN_X
    }

    fn bottom(&self) -> f32 {
        self.size.1 - MARGIN_BOTTOM
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = MARGIN_TOP;
    }

    fn at_top(&self) -> bool {
        self.y <= MARGIN_TOP + 0.5
    }

    /// Start a new page unless `h` more fits on this one.
    fn ensure(&mut self, h: f32) {
        if self.pages.is_empty() || (self.y + h > self.bottom() && !self.at_top()) {
            self.new_page();
        }
    }

    fn space(&mut self, h: f32) {
        if !self.at_top() {
            self.y += h;
        }
    }

    fn op(&mut self, op: Op) {
        if self.pages.is_empty() {
            self.new_page();
        }
        self.pages.last_mut().expect("a page").push(op);
    }

    fn page_index(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }

    fn text(&mut self, x: f32, baseline: f32, f: F, size: f32, color: Rgb, text: &str) {
        let text = self.fonts.clean(text);
        self.op(Op::Text { x, y: baseline, f, size, color, text });
    }

    // -----------------------------------------------------------------------
    // Text wrapping

    fn spans(&self, inlines: &[Inline], size: f32) -> Vec<Span> {
        inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) => Span { text: t.clone(), f: F::Regular, size, color: INK, link: None },
                Inline::Bold(t) => Span { text: t.clone(), f: F::Bold, size, color: INK, link: None },
                Inline::Italic(t) => Span { text: t.clone(), f: F::Regular, size, color: MUTED, link: None },
                // Links to other pages work inside the PDF; files are in the folder beside it.
                Inline::Link { text, href } if href.ends_with(".md") => Span { text: text.clone(), f: F::Medium, size, color: ACCENT, link: Some(href.clone()) },
                Inline::Link { text, .. } => Span { text: text.clone(), f: F::Medium, size, color: INK, link: None },
            })
            .collect()
    }

    /// Break spans into lines no wider than `width`.
    fn wrap(&self, spans: &[Span], width: f32) -> Vec<Vec<Placed>> {
        let mut lines: Vec<Vec<Placed>> = vec![Vec::new()];
        let mut x = 0.0f32;
        // Whether a space comes before the next word.
        let mut pending_space = false;
        for (si, s) in spans.iter().enumerate() {
            let text = self.fonts.clean(&s.text);
            let space_w = self.fonts.width(s.f, s.size, " ");
            let mut first = true;
            for word in text.split(' ') {
                if !first {
                    pending_space = true;
                }
                first = false;
                if word.is_empty() {
                    continue;
                }
                let mut word = word.to_string();
                loop {
                    let w = self.fonts.width(s.f, s.size, &word);
                    let gap = if pending_space && x > 0.0 { space_w } else { 0.0 };
                    if x + gap + w <= width || (x == 0.0 && w <= width) {
                        self.place(lines.last_mut().expect("a line"), &mut x, gap, w, si, &word);
                        pending_space = false;
                        break;
                    }
                    if x > 0.0 {
                        lines.push(Vec::new());
                        x = 0.0;
                        continue;
                    }
                    // A word wider than the whole line: break it.
                    let mut cut = word.len();
                    while cut > 1 && self.fonts.width(s.f, s.size, &word[..cut]) > width {
                        cut = word[..cut].char_indices().last().map(|(i, _)| i).unwrap_or(1).max(1);
                    }
                    let head = word[..cut].to_string();
                    let w = self.fonts.width(s.f, s.size, &head);
                    self.place(lines.last_mut().expect("a line"), &mut x, 0.0, w, si, &head);
                    word = word[cut..].to_string();
                    lines.push(Vec::new());
                    x = 0.0;
                    if word.is_empty() {
                        break;
                    }
                }
            }
        }
        lines.retain(|l| !l.is_empty());
        lines
    }

    fn place(&self, line: &mut Vec<Placed>, x: &mut f32, gap: f32, w: f32, span: usize, word: &str) {
        match line.last_mut() {
            // Same style as the last piece: join them.
            Some(last) if last.span == span => {
                if gap > 0.0 {
                    last.text.push(' ');
                }
                last.text.push_str(word);
                last.w = *x + gap + w - last.x;
            }
            _ => line.push(Placed { x: *x + gap, w, span, text: word.to_string() }),
        }
        *x += gap + w;
    }

    /// Draw wrapped lines at `x0`, flowing onto new pages as needed.
    fn draw_lines(&mut self, spans: &[Span], lines: Vec<Vec<Placed>>, x0: f32) {
        for line in lines {
            let size = line.iter().map(|p| spans[p.span].size).fold(0.0f32, f32::max).max(1.0);
            let lh = size * LEADING;
            self.ensure(lh);
            let baseline = self.y + (lh - size) / 2.0 + size * 0.8;
            for p in line {
                let s = &spans[p.span];
                self.op(Op::Text { x: x0 + p.x, y: baseline, f: s.f, size: s.size, color: s.color, text: p.text });
                if let Some(to) = &s.link {
                    self.op(Op::Link { x: x0 + p.x, y: self.y, w: p.w, h: lh, to: to.clone() });
                }
            }
            self.y += lh;
        }
    }

    fn paragraph(&mut self, spans: &[Span], x0: f32, width: f32) {
        let lines = self.wrap(spans, width);
        self.draw_lines(spans, lines, x0);
    }

    // -----------------------------------------------------------------------
    // Blocks

    fn chapter(&mut self, page: &Page, cover: bool) {
        self.current = page.path.clone();
        self.new_page();
        self.starts.insert(page.path.clone(), (self.page_index(), self.y));
        self.outline.push(((page.title.clone(), self.page_index(), self.y), Vec::new()));
        for b in &page.blocks {
            self.block(b, cover);
        }
    }

    fn block(&mut self, b: &Block, cover: bool) {
        let (x0, w) = (self.left(), self.width());
        match b {
            Block::Heading(1, t) => {
                let size = if cover { 36.0 } else { 26.0 };
                if cover {
                    self.y += 40.0;
                    self.text(x0, self.y + 10.0, F::Medium, SMALL, ACCENT, "HOUSE MANUAL");
                    self.y += 22.0;
                }
                let spans = [Span { text: t.clone(), f: F::Serif, size, color: INK, link: None }];
                self.paragraph(&spans, x0, w);
                self.y += 4.0;
                self.op(Op::Line { x1: x0, y1: self.y, x2: x0 + w, y2: self.y, color: RULE, width: 0.8 });
                self.y += 14.0;
            }
            Block::Heading(level, t) => {
                let (size, before, f) = match level {
                    2 => (14.0, 16.0, F::Bold),
                    3 => (11.5, 12.0, F::Bold),
                    _ => (10.0, 9.0, F::Bold),
                };
                self.space(before);
                // Keep a heading with a few lines of what follows it.
                self.ensure(size * LEADING + BODY * LEADING * 3.0);
                if *level == 2 {
                    let (p, y) = (self.page_index(), self.y);
                    if let Some(ch) = self.outline.last_mut() {
                        ch.1.push((t.clone(), p, y));
                    }
                }
                let color = if *level >= 4 { MUTED } else { INK };
                let spans = [Span { text: t.clone(), f, size, color, link: None }];
                self.paragraph(&spans, x0, w);
                self.y += 3.0;
            }
            Block::Para(inlines) => {
                let spans = self.spans(inlines, BODY);
                self.paragraph(&spans, x0, w);
                self.y += 6.0;
            }
            Block::Lines(t) => {
                for line in t.trim().lines() {
                    if line.trim().is_empty() {
                        self.y += BODY * 0.6;
                        continue;
                    }
                    let spans = [Span { text: line.trim_end().to_string(), f: F::Regular, size: BODY, color: INK, link: None }];
                    self.paragraph(&spans, x0, w);
                }
                self.y += 6.0;
            }
            Block::Fields(pairs) => self.fields(pairs),
            Block::Bullets(items) => {
                for item in items {
                    let spans = self.spans(item, BODY);
                    let lines = self.wrap(&spans, w - 14.0 - 30.0);
                    let lh = BODY * LEADING;
                    self.ensure(lh);
                    let baseline = self.y + (lh - BODY) / 2.0 + BODY * 0.8;
                    self.text(x0 + 2.0, baseline, F::Regular, BODY, MUTED, "•");
                    // In the contents, the page each one starts on.
                    if let Some(Inline::Link { href, .. }) = item.first() {
                        if href.ends_with(".md") {
                            self.op(Op::PageNumber { x: x0 + w, y: baseline, to: href.clone() });
                        }
                    }
                    self.draw_lines(&spans, lines, x0 + 14.0);
                    self.y += 2.0;
                }
                self.y += 5.0;
            }
            Block::Table { head, rows } => self.table(head, rows),
            Block::Image { src, alt } => self.image(src, alt),
        }
    }

    fn fields(&mut self, pairs: &[(String, String)]) {
        let (x0, w) = (self.left(), self.width());
        // One label column everywhere, so fields line up from item to item.
        let label_w = 112.0;
        for (k, v) in pairs {
            let lh = BODY * LEADING;
            let label = [Span { text: k.clone(), f: F::Regular, size: 9.0, color: MUTED, link: None }];
            let label_lines = self.wrap(&label, label_w - 14.0);
            let mut value_lines = Vec::new();
            let spans = [Span { text: String::new(), f: F::Regular, size: BODY, color: INK, link: None }];
            let mut all = Vec::new();
            for line in v.lines() {
                let s = [Span { text: line.to_string(), ..spans[0].clone() }];
                value_lines.push(self.wrap(&s, w - label_w));
                all.push(s[0].clone());
            }
            self.ensure(lh);
            let top = self.y;
            let start_page = self.page_index();
            let baseline = top + (lh - BODY) / 2.0 + BODY * 0.8;
            for (i, l) in label_lines.into_iter().enumerate() {
                for p in l {
                    self.op(Op::Text { x: x0 + p.x, y: baseline + i as f32 * lh, f: F::Regular, size: 9.0, color: MUTED, text: p.text });
                }
            }
            for (s, lines) in all.iter().zip(value_lines) {
                if lines.is_empty() {
                    self.y += lh;
                    continue;
                }
                self.draw_lines(std::slice::from_ref(s), lines, x0 + label_w);
            }
            if self.page_index() == start_page {
                self.y = self.y.max(top + lh);
            }
            self.y += 2.5;
        }
        self.y += 6.0;
    }

    fn table(&mut self, head: &[String], rows: &[Vec<String>]) {
        let (x0, w) = (self.left(), self.width());
        let n = head.len().max(1);
        let pad = 4.0;
        let cell = |s: &str, f: F| Span { text: s.to_string(), f, size: SMALL, color: INK, link: None };
        // Natural (one line) and smallest (longest word) widths per column.
        let mut natural = vec![0.0f32; n];
        let mut least = vec![0.0f32; n];
        for (i, h) in head.iter().enumerate().take(n) {
            natural[i] = self.fonts.width(F::Bold, SMALL, h);
            least[i] = h.split_whitespace().map(|word| self.fonts.width(F::Bold, SMALL, word)).fold(0.0, f32::max);
        }
        for r in rows {
            for (i, c) in r.iter().enumerate().take(n) {
                let c = self.fonts.clean(c);
                natural[i] = natural[i].max(c.lines().map(|l| self.fonts.width(F::Regular, SMALL, l)).fold(0.0, f32::max));
                least[i] = least[i].max(c.split_whitespace().map(|word| self.fonts.width(F::Regular, SMALL, word)).fold(0.0, f32::max).min(90.0));
            }
        }
        let avail = w - pad * 2.0 * n as f32;
        let total: f32 = natural.iter().sum();
        let widths: Vec<f32> = if total <= avail {
            natural.iter().map(|x| x + (avail - total) * (x / total.max(1.0))).collect()
        } else {
            let min_total: f32 = least.iter().sum();
            if min_total >= avail {
                least.iter().map(|x| x * avail / min_total.max(1.0)).collect()
            } else {
                let extra: f32 = natural.iter().zip(&least).map(|(a, b)| a - b).sum();
                natural.iter().zip(&least).map(|(a, b)| b + (a - b) / extra.max(1.0) * (avail - min_total)).collect()
            }
        };
        let lh = SMALL * LEADING;
        let draw_row = |l: &mut Self, cells: &[String], f: F, shade: bool| {
            let wrapped: Vec<(Span, Vec<Vec<Placed>>)> = (0..n)
                .map(|i| {
                    let text = cells.get(i).map(|s| s.replace('\n', " ")).unwrap_or_default();
                    let s = cell(&text, f);
                    let lines = l.wrap(std::slice::from_ref(&s), widths[i]);
                    (s, lines)
                })
                .collect();
            let lines = wrapped.iter().map(|(_, l)| l.len()).max().unwrap_or(1).max(1);
            let h = lines as f32 * lh + pad;
            l.ensure(h);
            if shade {
                l.op(Op::Rect { x: x0, y: l.y, w, h, fill: SHADE });
            }
            let mut x = x0;
            for (i, (s, ls)) in wrapped.into_iter().enumerate() {
                for (j, line) in ls.into_iter().enumerate() {
                    let baseline = l.y + pad / 2.0 + j as f32 * lh + (lh - SMALL) / 2.0 + SMALL * 0.8;
                    for p in line {
                        l.op(Op::Text { x: x + pad + p.x, y: baseline, f: s.f, size: SMALL, color: if shade { MUTED } else { INK }, text: p.text });
                    }
                }
                x += widths[i] + pad * 2.0;
            }
            l.y += h;
            l.op(Op::Line { x1: x0, y1: l.y, x2: x0 + w, y2: l.y, color: RULE, width: 0.5 });
        };
        let row_h = |l: &Self, r: &[String], f: F| {
            (0..n).map(|i| l.wrap(&[cell(&r.get(i).cloned().unwrap_or_default().replace('\n', " "), f)], widths[i]).len()).max().unwrap_or(1).max(1) as f32 * lh + pad
        };
        // The header stays with the first row.
        let first = rows.first().map(|r| row_h(self, r, F::Regular)).unwrap_or(0.0);
        self.ensure(row_h(self, head, F::Bold) + first);
        draw_row(self, head, F::Bold, true);
        for r in rows {
            let page = self.page_index();
            // Repeat the header when a row moves to a new page.
            let h = row_h(self, r, F::Regular);
            self.ensure(h);
            if self.page_index() != page {
                draw_row(self, head, F::Bold, true);
            }
            draw_row(self, r, F::Regular, false);
        }
        self.y += 10.0;
    }

    fn image(&mut self, src: &str, alt: &str) {
        let (x0, w) = (self.left(), self.width());
        let content_h = self.bottom() - MARGIN_TOP;
        if let Some(d) = self.drawings.get(src) {
            let (_, _, vw, vh) = d.view();
            // At most about 1:70, so a small shed isn't blown up to fill the page.
            let scale = (w / vw as f32).min(content_h * 0.62 / vh as f32).min(40.0);
            let (dw, dh) = (vw as f32 * scale, vh as f32 * scale);
            self.space(4.0);
            self.ensure(dh);
            let idx = self.drawing_list.len();
            self.drawing_list.push(d);
            let x = x0 + (w - dw) / 2.0;
            self.op(Op::Drawing { x, y: self.y, scale, drawing: idx });
            self.op(Op::Line { x1: x, y1: self.y + dh, x2: x + dw, y2: self.y + dh, color: RULE, width: 0.5 });
            self.y += dh + 10.0;
            return;
        }
        let image = self.images.get(src).and_then(|bytes| load_image(bytes));
        match image {
            Some(img) => {
                let (pw, ph) = img.size();
                let scale = (w / pw as f32).min(content_h * 0.45 / ph as f32).min(1.0);
                let (iw, ih) = (pw as f32 * scale, ph as f32 * scale);
                self.space(4.0);
                self.ensure(ih + SMALL * LEADING);
                let idx = self.image_list.len();
                self.image_list.push(img);
                self.op(Op::Image { x: x0, y: self.y, w: iw, h: ih, image: idx });
                self.y += ih + 2.0;
                let spans = [Span { text: alt.to_string(), f: F::Regular, size: SMALL, color: MUTED, link: None }];
                self.paragraph(&spans, x0, w);
                self.y += 8.0;
            }
            None => {
                let spans = [Span { text: format!("{alt} (in the files folder)"), f: F::Regular, size: BODY, color: MUTED, link: None }];
                self.paragraph(&spans, x0, w);
                self.y += 4.0;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Writing the PDF

    fn finish(self, title: &str) -> Result<Vec<u8>, String> {
        let mut doc = Document::new();
        let page_size = Size::from_wh(self.size.0, self.size.1).ok_or("page size")?;
        let total = self.pages.len();
        let dest = |starts: &BTreeMap<String, (usize, f32)>, to: &str| starts.get(to).map(|(p, y)| XyzDestination::new(*p, Point::from_xy(0.0, *y - 20.0)));
        for (i, ops) in self.pages.iter().enumerate() {
            let mut page = doc.start_page_with(PageSettings::new(page_size));
            let mut links = Vec::new();
            {
                let mut s = page.surface();
                for op in ops {
                    match op {
                        Op::Text { x, y, f, size, color, text } => self.draw_text(&mut s, *x, *y, *f, *size, *color, text),
                        Op::Rect { x, y, w, h, fill } => {
                            if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                                let mut pb = PathBuilder::new();
                                pb.push_rect(r);
                                if let Some(path) = pb.finish() {
                                    set_fill(&mut s, Some(*fill));
                                    s.set_stroke(None);
                                    s.draw_path(&path);
                                }
                            }
                        }
                        Op::Line { x1, y1, x2, y2, color, width } => {
                            let mut pb = PathBuilder::new();
                            pb.move_to(*x1, *y1);
                            pb.line_to(*x2, *y2);
                            if let Some(path) = pb.finish() {
                                set_fill(&mut s, None);
                                set_stroke(&mut s, *color, *width, LineCap::Butt, None);
                                s.draw_path(&path);
                            }
                        }
                        Op::Drawing { x, y, scale, drawing } => self.draw_drawing(&mut s, *x, *y, *scale, self.drawing_list[*drawing]),
                        Op::Image { x, y, w, h, image } => {
                            if let Some(size) = Size::from_wh(*w, *h) {
                                s.push_transform(&Transform::from_translate(*x, *y));
                                s.draw_image(self.image_list[*image].clone(), size);
                                s.pop();
                            }
                        }
                        Op::Link { x, y, w, h, to } => {
                            if let (Some(rect), Some(d)) = (Rect::from_xywh(*x, *y, *w, *h), dest(&self.starts, to)) {
                                links.push(Annotation::new_link(LinkAnnotation::new(rect, Target::Destination(d.into())), None));
                            }
                        }
                        Op::PageNumber { x, y, to } => {
                            if let Some((p, _)) = self.starts.get(to) {
                                let n = (p + 1).to_string();
                                let w = self.fonts.width(F::Regular, BODY, &n);
                                self.draw_text(&mut s, x - w, *y, F::Regular, BODY, MUTED, &n);
                            }
                        }
                    }
                }
                // Footer.
                if i > 0 {
                    let y = self.size.1 - 32.0;
                    self.draw_text(&mut s, MARGIN_X, y, F::Regular, 8.0, MUTED, &self.fonts.clean(title));
                    let n = format!("{} of {total}", i + 1);
                    let w = self.fonts.width(F::Regular, 8.0, &n);
                    self.draw_text(&mut s, self.size.0 - MARGIN_X - w, y, F::Regular, 8.0, MUTED, &n);
                }
                s.finish();
            }
            for a in links {
                page.add_annotation(a);
            }
            page.finish();
        }
        let mut outline = Outline::new();
        for ((t, p, y), sections) in &self.outline {
            let mut node = OutlineNode::new(t.clone(), XyzDestination::new(*p, Point::from_xy(0.0, (y - 20.0).max(0.0))));
            for (st, sp, sy) in sections {
                node.push_child(OutlineNode::new(st.clone(), XyzDestination::new(*sp, Point::from_xy(0.0, (sy - 20.0).max(0.0)))));
            }
            outline.push_child(node);
        }
        doc.set_outline(outline);
        doc.set_metadata(Metadata::new().title(format!("{title} · House manual")).creator("Abode".to_string()).language("en".to_string()));
        doc.finish().map_err(|e| format!("couldn't write the PDF: {e:?}"))
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(&self, s: &mut Surface, x: f32, y: f32, f: F, size: f32, color: Rgb, text: &str) {
        if text.is_empty() {
            return;
        }
        set_fill(s, Some(color));
        s.set_stroke(None);
        s.draw_text(Point::from_xy(x, y), self.fonts.pdf[Fonts::i(f)].clone(), size, text, false, TextDirection::Auto);
    }

    /// A plan drawing, scaled from metres to points with its top-left at (x, y).
    fn draw_drawing(&self, s: &mut Surface, x: f32, y: f32, scale: f32, d: &Drawing) {
        let (vx, vy, vw, vh) = d.view();
        s.push_transform(&Transform::from_row(scale, 0.0, 0.0, scale, x - vx as f32 * scale, y - vy as f32 * scale));
        if let Some(r) = Rect::from_xywh(vx as f32, vy as f32, vw as f32, vh as f32) {
            let mut pb = PathBuilder::new();
            pb.push_rect(r);
            if let Some(path) = pb.finish() {
                set_fill(s, Some([255, 255, 255]));
                s.set_stroke(None);
                s.draw_path(&path);
            }
        }
        for shape in &d.shapes {
            match shape {
                Shape::Polygon { points, fill, stroke, width } => {
                    let Some(path) = poly_path(points, true) else { continue };
                    set_fill(s, *fill);
                    match stroke {
                        Some(c) if *width > 0.0 => set_stroke(s, *c, *width as f32, LineCap::Butt, None),
                        _ => s.set_stroke(None),
                    }
                    s.draw_path(&path);
                }
                Shape::Polyline { points, stroke, width, cap, dash } => {
                    let Some(path) = poly_path(points, false) else { continue };
                    set_fill(s, None);
                    let cap = match cap {
                        Cap::Round => LineCap::Round,
                        Cap::Square => LineCap::Square,
                    };
                    set_stroke(s, *stroke, *width as f32, cap, dash.map(|(a, b)| (a as f32, b as f32)));
                    s.draw_path(&path);
                }
                Shape::Circle { center, radius, fill, stroke, width } => {
                    let Some(path) = circle_path(center.x as f32, center.y as f32, *radius as f32) else { continue };
                    set_fill(s, Some(*fill));
                    set_stroke(s, *stroke, *width as f32, LineCap::Butt, None);
                    s.draw_path(&path);
                }
                Shape::Text { at, size, color, bold, text } => {
                    let f = if *bold { F::Bold } else { F::Regular };
                    let size = *size as f32;
                    let text = self.fonts.clean(text);
                    let w = self.fonts.width(f, size, &text);
                    self.draw_text(s, at.x as f32 - w / 2.0, at.y as f32 + size * 0.35, f, size, *color, &text);
                }
            }
        }
        s.pop();
    }
}

fn set_fill(s: &mut Surface, color: Option<Rgb>) {
    s.set_fill(color.map(|[r, g, b]| Fill { paint: rgb::Color::new(r, g, b).into(), opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
}

fn set_stroke(s: &mut Surface, [r, g, b]: Rgb, width: f32, cap: LineCap, dash: Option<(f32, f32)>) {
    s.set_stroke(Some(Stroke {
        paint: rgb::Color::new(r, g, b).into(),
        width,
        line_cap: cap,
        line_join: LineJoin::Round,
        dash: dash.map(|(a, b)| StrokeDash { array: vec![a, b], offset: 0.0 }),
        ..Default::default()
    }));
}

fn poly_path(points: &[crate::geometry::Vec2], close: bool) -> Option<krilla::geom::Path> {
    let (first, rest) = points.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.x as f32, first.y as f32);
    for p in rest {
        pb.line_to(p.x as f32, p.y as f32);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

fn circle_path(cx: f32, cy: f32, r: f32) -> Option<krilla::geom::Path> {
    // Four cubic Béziers.
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.finish()
}

/// A photo for the PDF: JPEG and PNG go in as they are; anything else the
/// image library can read is converted (and shrunk if it's huge).
fn load_image(bytes: &[u8]) -> Option<Image> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        if let Ok(img) = Image::from_jpeg(bytes.to_vec().into(), true) {
            return Some(img);
        }
    }
    if bytes.starts_with(b"\x89PNG") {
        if let Ok(img) = Image::from_png(bytes.to_vec().into(), true) {
            return Some(img);
        }
    }
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width().max(img.height()) > 2000 { img.thumbnail(2000, 2000) } else { img };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(Image::from_rgba8(rgba.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::doc::{link, para, text};

    #[test]
    fn wraps_and_breaks_long_words() {
        let fonts = Fonts::load().unwrap();
        let d = BTreeMap::new();
        let i = BTreeMap::new();
        let l = Layout::new(&fonts, (595.0, 842.0), &d, &i);
        let spans = l.spans(&[text("The quick brown fox jumps over the lazy dog ".repeat(6))], BODY);
        let lines = l.wrap(&spans, 200.0);
        assert!(lines.len() > 3);
        assert!(lines.iter().all(|line| line.iter().all(|p| p.x + p.w <= 200.5)));
        let long = l.spans(&[text("x".repeat(400))], BODY);
        assert!(l.wrap(&long, 100.0).len() > 2);
    }

    #[test]
    fn fonts_cover_what_the_export_writes() {
        let fonts = Fonts::load().unwrap();
        for s in ["16.0 m²", "Tomato ‘Sungold’", "2024–2026", "4.00 m × 4.00 m", "Water heater · Utility", "“Use”", "•"] {
            assert_eq!(fonts.clean(s), s, "{s}");
        }
        assert_eq!(fonts.clean("a\u{7}b"), "ab");
    }

    #[test]
    fn renders_a_pdf() {
        let mut readme = Page::new("README.md", "Maple House");
        readme.push(Block::Bullets(vec![vec![link("Kitchen", "pages/kitchen.md")]]));
        let mut kitchen = Page::new("pages/kitchen.md", "Kitchen");
        kitchen.push(Block::Heading(2, "Items".into()));
        kitchen.push(para("A fridge. ".repeat(400)));
        kitchen.push(Block::Fields(vec![("Make".into(), "LG".into()), ("Notes".into(), "Line one\nLine two".into())]));
        kitchen.push(Block::Table { head: vec!["Date".into(), "What".into()], rows: (0..80).map(|i| vec![format!("Day {i}"), "Cleaned the coils and checked the seals".into()]).collect() });
        kitchen.push(Block::Image { src: "files/missing.jpg".into(), alt: "missing.jpg".into() });
        let pdf = render(&[readme, kitchen], &BTreeMap::new(), &BTreeMap::new(), Settings { title: "Maple House", letter: false }).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 2000);
    }

    #[test]
    fn embeds_photos() {
        let photo = image::RgbImage::from_fn(64, 48, |x, y| image::Rgb([x as u8 * 4, y as u8 * 5, 120]));
        let encode = |format| {
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(photo.clone()).write_to(&mut out, format).unwrap();
            out.into_inner()
        };
        let mut images = BTreeMap::new();
        images.insert("files/a.png".to_string(), encode(image::ImageFormat::Png));
        images.insert("files/b.jpg".to_string(), encode(image::ImageFormat::Jpeg));
        images.insert("files/c.bmp".to_string(), encode(image::ImageFormat::Bmp));
        assert!(images.values().all(|b| load_image(b).is_some()));
        assert!(load_image(b"not an image").is_none());
        let mut page = Page::new("README.md", "Photos");
        for src in images.keys() {
            page.push(Block::Image { src: src.clone(), alt: src.clone() });
        }
        let pdf = render(&[page], &BTreeMap::new(), &images, Settings { title: "Photos", letter: true }).unwrap();
        // Three images in the file.
        assert_eq!(pdf.windows(b"/Subtype/Image".len()).filter(|w| *w == b"/Subtype/Image").count(), 3);
    }
}
