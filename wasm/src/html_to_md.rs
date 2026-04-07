use ego_tree::NodeRef;
use scraper::{Html, Node, Selector};
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::LazyLock;
use url::Url;

fn sel_title() -> Option<Selector> {
    Selector::parse("title").ok()
}
fn sel_body() -> Option<Selector> {
    Selector::parse("body").ok()
}

pub fn html_to_markdown(html: &str, base_url: Option<&str>, selector: &str) -> String {
    let document = Html::parse_document(html);
    let title = sel_title()
        .and_then(|s| {
            document
                .select(&s)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
        })
        .filter(|s| !s.is_empty());

    let mut writer = MdWriter::new(base_url);

    if !selector.is_empty() {
        if let Ok(sel) = Selector::parse(selector) {
            let mut first = true;
            for el in document.select(&sel) {
                if !first {
                    ensure_double_nl(&mut writer.out);
                }
                first = false;
                writer.walk(*el);
            }
        }
    } else if let Some(body_sel) = sel_body() {
        if let Some(body) = document.select(&body_sel).next() {
            writer.walk(*body);
        } else {
            writer.walk_children(document.tree.root());
        }
    } else {
        writer.walk_children(document.tree.root());
    }

    let md = writer.finish();
    if selector.is_empty() {
        title.map(|t| format!("# {t}\n\n{md}")).unwrap_or(md)
    } else {
        md
    }
}

struct ListCtx {
    ordered: bool,
    counter: u32,
}

struct MdWriter {
    out: String,
    list_stack: Vec<ListCtx>,
    in_pre: bool,
    in_link: bool,
    strip_nav: bool,
    base_url: Option<Url>,
    seen_hrefs: HashSet<String>,
}

impl MdWriter {
    fn new(base_url: Option<&str>) -> Self {
        Self {
            out: String::new(),
            list_stack: Vec::new(),
            in_pre: false,
            in_link: false,
            strip_nav: true,
            base_url: base_url.and_then(|u| Url::parse(u).ok()),
            seen_hrefs: HashSet::new(),
        }
    }

    fn resolve_href(&self, href: &str) -> String {
        match &self.base_url {
            Some(b) => b
                .join(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string()),
            None => href.to_string(),
        }
    }

    fn walk(&mut self, node: NodeRef<'_, Node>) {
        match node.value() {
            Node::Text(text) => {
                let s = text.text.as_ref();
                if self.in_pre {
                    self.out.push_str(s);
                } else {
                    let c = collapse_ws(s);
                    if !c.is_empty() {
                        self.out.push_str(&c);
                    }
                }
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                match tag {
                    "script" | "style" | "noscript" => {}
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        if self.in_link {
                            self.walk_children(node);
                        } else {
                            let lvl = tag.as_bytes()[1] - b'0';
                            ensure_nl(&mut self.out);
                            for _ in 0..lvl {
                                self.out.push('#');
                            }
                            self.out.push(' ');
                            self.walk_children(node);
                            self.out.push('\n');
                        }
                    }
                    "header" | "footer" | "nav" if self.strip_nav => {}
                    "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                        ensure_nl(&mut self.out);
                        self.walk_children(node);
                        ensure_double_nl(&mut self.out);
                    }
                    "br" => self.out.push('\n'),
                    "hr" => {
                        ensure_nl(&mut self.out);
                        self.out.push_str("---\n");
                    }
                    "a" => {
                        let href = el.attr("href").unwrap_or_default();
                        let resolved = self.resolve_href(href);
                        if !resolved.is_empty() && self.seen_hrefs.contains(&resolved) {
                            return;
                        }
                        let was = self.in_link;
                        self.in_link = true;
                        let before = self.out.len();
                        self.out.push('[');
                        self.walk_children(node);
                        let txt = self.out[before + 1..].trim().replace(['*', '_', '`'], "");
                        if txt.trim().is_empty() {
                            if let Some(lbl) = el
                                .attr("title")
                                .or_else(|| el.attr("aria-label"))
                                .filter(|s| !s.trim().is_empty())
                            {
                                self.out.truncate(before);
                                self.out.push('[');
                                self.out.push_str(lbl.trim());
                            } else {
                                self.out.truncate(before);
                                self.in_link = was;
                                return;
                            }
                        }
                        let flat = self.out[before + 1..]
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        self.out.truncate(before + 1);
                        self.out.push_str(&flat);
                        self.out.push_str("](");
                        self.out.push_str(&resolved);
                        self.out.push(')');
                        if !resolved.is_empty() {
                            self.seen_hrefs.insert(resolved);
                        }
                        self.in_link = was;
                    }
                    "strong" | "b" => {
                        let b = self.out.len();
                        self.out.push_str("**");
                        self.walk_children(node);
                        if self.out[b + 2..].trim().is_empty() {
                            self.out.truncate(b);
                        } else {
                            self.out.push_str("**");
                        }
                    }
                    "em" | "i" => {
                        let b = self.out.len();
                        self.out.push('*');
                        self.walk_children(node);
                        if self.out[b + 1..].trim().is_empty() {
                            self.out.truncate(b);
                        } else {
                            self.out.push('*');
                        }
                    }
                    "code" if self.in_pre => self.walk_children(node),
                    "code" => {
                        self.out.push('`');
                        self.walk_children(node);
                        self.out.push('`');
                    }
                    "pre" => {
                        self.in_pre = true;
                        ensure_nl(&mut self.out);
                        self.out.push_str("```\n");
                        self.walk_children(node);
                        ensure_nl(&mut self.out);
                        self.out.push_str("```\n");
                        self.in_pre = false;
                    }
                    "blockquote" => {
                        ensure_nl(&mut self.out);
                        let mut inner = MdWriter::new(None);
                        inner.strip_nav = self.strip_nav;
                        inner.in_pre = self.in_pre;
                        inner.base_url = self.base_url.clone();
                        inner.walk_children(node);
                        for line in inner.out.lines() {
                            self.out.push_str("> ");
                            self.out.push_str(line);
                            self.out.push('\n');
                        }
                    }
                    "ul" => {
                        self.list_stack.push(ListCtx {
                            ordered: false,
                            counter: 0,
                        });
                        ensure_nl(&mut self.out);
                        self.walk_children(node);
                        self.list_stack.pop();
                        ensure_nl(&mut self.out);
                    }
                    "ol" => {
                        self.list_stack.push(ListCtx {
                            ordered: true,
                            counter: 0,
                        });
                        ensure_nl(&mut self.out);
                        self.walk_children(node);
                        self.list_stack.pop();
                        ensure_nl(&mut self.out);
                    }
                    "li" => {
                        let b = self.out.len();
                        ensure_nl(&mut self.out);
                        let d = self.list_stack.len().saturating_sub(1);
                        for _ in 0..d {
                            self.out.push_str("  ");
                        }
                        if let Some(ctx) = self.list_stack.last_mut() {
                            if ctx.ordered {
                                ctx.counter += 1;
                                let _ = write!(self.out, "{}. ", ctx.counter);
                            } else {
                                self.out.push_str("- ");
                            }
                        } else {
                            self.out.push_str("- ");
                        }
                        let m = self.out.len();
                        self.walk_children(node);
                        if self.out[m..].trim().is_empty() {
                            self.out.truncate(b);
                        }
                    }
                    "img" => {
                        let alt = el.attr("alt").unwrap_or_default();
                        let src = el.attr("src").unwrap_or_default();
                        if !src.is_empty() {
                            let r = self.resolve_href(src);
                            let _ = write!(self.out, "![{alt}]({r})");
                        }
                    }
                    "table" => {
                        ensure_nl(&mut self.out);
                        self.write_table(node);
                        self.out.push('\n');
                    }
                    "thead" | "tbody" | "tfoot" | "tr" | "th" | "td" | "caption" | "colgroup"
                    | "col" => self.walk_children(node),
                    _ => self.walk_children(node),
                }
            }
            Node::Document => self.walk_children(node),
            _ => {}
        }
    }

    fn walk_children(&mut self, node: NodeRef<'_, Node>) {
        for c in node.children() {
            self.walk(c);
        }
    }

    fn write_table(&mut self, tbl: NodeRef<'_, Node>) {
        let rows = collect_rows(tbl);
        if rows.is_empty() {
            return;
        }
        let cols = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let hdr_count = rows.iter().filter(|r| r.0).count();
        let sep_after = if hdr_count > 0 { hdr_count } else { 1 };
        for (i, (_, cells)) in rows.iter().enumerate() {
            self.out.push('|');
            for c in 0..cols {
                let cell = cells.get(c).map(|s| s.as_str()).unwrap_or("");
                let _ = write!(self.out, " {cell} |");
            }
            self.out.push('\n');
            if i + 1 == sep_after {
                self.out.push('|');
                for _ in 0..cols {
                    self.out.push_str("---|");
                }
                self.out.push('\n');
            }
        }
    }

    fn finish(self) -> String {
        normalize_ws(&decode_entities(&self.out))
    }
}

fn collect_rows(node: NodeRef<'_, Node>) -> Vec<(bool, Vec<String>)> {
    let mut rows = Vec::new();
    collect_rows_rec(node, &mut rows, false);
    rows
}
fn collect_rows_rec(node: NodeRef<'_, Node>, rows: &mut Vec<(bool, Vec<String>)>, in_thead: bool) {
    for child in node.children() {
        if let Node::Element(el) = child.value() {
            match el.name.local.as_ref() {
                "thead" => collect_rows_rec(child, rows, true),
                "tbody" | "tfoot" => collect_rows_rec(child, rows, false),
                "tr" => {
                    let mut cells = Vec::new();
                    let mut is_hdr = in_thead;
                    for cn in child.children() {
                        if let Node::Element(ce) = cn.value() {
                            let ct = ce.name.local.as_ref();
                            if ct == "th" || ct == "td" {
                                if ct == "th" {
                                    is_hdr = true;
                                }
                                cells.push(cell_text(cn));
                            }
                        }
                    }
                    rows.push((is_hdr, cells));
                }
                _ => collect_rows_rec(child, rows, in_thead),
            }
        }
    }
}
fn cell_text(node: NodeRef<'_, Node>) -> String {
    let mut t = String::new();
    for n in node.descendants() {
        if let Node::Text(tx) = n.value() {
            t.push_str(tx.text.as_ref());
        }
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collapse_ws(s: &str) -> String {
    let mut o = String::new();
    let mut pw = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !pw && !o.is_empty() {
                o.push(' ');
            }
            pw = true;
        } else {
            pw = false;
            o.push(ch);
        }
    }
    o
}
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}
fn normalize_ws(s: &str) -> String {
    let mut o = String::new();
    let mut bc = 0;
    let mut started = false;
    for line in s.lines() {
        let t = line.trim_end();
        if t.is_empty() {
            if started && bc < 1 {
                o.push('\n');
            }
            bc += 1;
        } else {
            started = true;
            bc = 0;
            if !o.is_empty() && !o.ends_with('\n') {
                o.push('\n');
            }
            o.push_str(t);
            o.push('\n');
        }
    }
    let tl = o.trim_end_matches('\n').len();
    o.truncate(tl);
    o
}
fn ensure_nl(s: &mut String) {
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
}
fn ensure_double_nl(s: &mut String) {
    ensure_nl(s);
    if !s.ends_with("\n\n") {
        s.push('\n');
    }
}
