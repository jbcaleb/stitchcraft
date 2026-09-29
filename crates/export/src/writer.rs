//! A small indentation-aware source builder, plus the literal escaping every
//! emitter needs.
//!
//! Generated Java is read by people - they open it when something goes wrong,
//! and they keep it when they outgrow the editor - so indentation and blank
//! lines are worth the bookkeeping.

/// Builds a source file a line at a time, tracking indentation.
#[derive(Debug, Default)]
pub struct Source {
    buffer: String,
    depth: usize,
}

impl Source {
    pub fn new() -> Self {
        Self::default()
    }

    /// One line at the current depth. An empty `text` writes a bare newline
    /// rather than trailing whitespace.
    pub fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if !text.is_empty() {
            for _ in 0..self.depth {
                self.buffer.push_str("    ");
            }
            self.buffer.push_str(text);
        }
        self.buffer.push('\n');
    }

    pub fn blank(&mut self) {
        self.buffer.push('\n');
    }

    /// Several lines at the current depth.
    pub fn lines<S: AsRef<str>>(&mut self, lines: impl IntoIterator<Item = S>) {
        for line in lines {
            self.line(line);
        }
    }

    /// Writes `header` and indents what follows.
    pub fn open(&mut self, header: impl AsRef<str>) {
        self.line(header);
        self.depth += 1;
    }

    /// Un-indents and writes `footer`.
    pub fn close(&mut self, footer: impl AsRef<str>) {
        self.depth = self.depth.saturating_sub(1);
        self.line(footer);
    }

    /// Closes one block and opens the next in a single line - `} else {`.
    pub fn pivot(&mut self, text: impl AsRef<str>) {
        self.depth = self.depth.saturating_sub(1);
        self.line(text);
        self.depth += 1;
    }

    /// `header { ... }` - the shape almost every Java construct has.
    pub fn braced(&mut self, header: impl AsRef<str>, body: impl FnOnce(&mut Self)) {
        self.open(format!("{} {{", header.as_ref()));
        body(self);
        self.close("}");
    }

    /// A `/** ... */` comment, wrapped at a readable width.
    pub fn doc(&mut self, text: &str) {
        self.line("/**");
        for line in wrap(text, 76) {
            if line.is_empty() {
                self.line(" *");
            } else {
                self.line(format!(" * {line}"));
            }
        }
        self.line(" */");
    }

    /// A `//` comment, wrapped.
    pub fn comment(&mut self, text: &str) {
        for line in wrap(text, 76) {
            if line.is_empty() {
                self.line("//");
            } else {
                self.line(format!("// {line}"));
            }
        }
    }

    pub fn finish(self) -> String {
        self.buffer
    }
}

/// Greedy word wrap. Paragraphs are separated by a blank line, which survives.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.split("\n\n") {
        if !out.is_empty() {
            out.push(String::new());
        }
        let mut current = String::new();
        let mut used = 0usize;
        for word in paragraph.split_whitespace() {
            // Measured in characters, not bytes: a line of box-drawing rules is
            // three bytes a glyph and would otherwise wrap at a third of `width`.
            let length = word.chars().count();
            if current.is_empty() {
                current.push_str(word);
                used = length;
            } else if used + 1 + length <= width {
                current.push(' ');
                current.push_str(word);
                used += 1 + length;
            } else {
                out.push(std::mem::take(&mut current));
                current.push_str(word);
                used = length;
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

/// A Java string literal, escaped. Anything outside printable ASCII becomes a
/// `\u` escape, so the generated file is ASCII whatever the canvas held.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_graphic() || c == ' ' => out.push(c),
            c => {
                // Java literals are UTF-16, so anything outside the BMP needs
                // both halves of its surrogate pair spelled out.
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// A Java `double` literal. Non-finite values cannot be written as literals, so
/// they come out as the `Double` constants that mean the same thing.
pub fn double(value: f64) -> String {
    if value.is_nan() {
        return "Double.NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "Double.NEGATIVE_INFINITY".to_string()
        } else {
            "Double.POSITIVE_INFINITY".to_string()
        };
    }
    if value == value.trunc() && value.abs() < 1e15 {
        // `4.0D` rather than `4D`: the decimal point is what makes it readable
        // as a number rather than a count.
        format!("{value:.1}D")
    } else {
        format!("{value}D")
    }
}

/// A Java `int` literal, clamping a value that cannot be one. Slots that only
/// accept whole numbers still hold a `f64`, and a hand-edited save can put
/// anything in one.
pub fn int(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    format!("{}", value.trunc().clamp(i32::MIN as f64, i32::MAX as f64) as i32)
}

/// A Java `float` literal, for the many Minecraft signatures that take one.
pub fn float(value: f64) -> String {
    if !value.is_finite() {
        return "0.0F".to_string();
    }
    let narrowed = value as f32;
    // `2.0F` rather than `2F`, to match `double` and read as a measurement.
    if narrowed == narrowed.trunc() && narrowed.abs() < 1e7 {
        format!("{narrowed:.1}F")
    } else {
        format!("{narrowed}F")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braced_blocks_nest() {
        let mut source = Source::new();
        source.braced("class A", |source| {
            source.braced("void run()", |source| source.line("call();"));
        });
        assert_eq!(
            source.finish(),
            "class A {\n    void run() {\n        call();\n    }\n}\n"
        );
    }

    #[test]
    fn string_literals_escape_what_java_needs_escaped() {
        assert_eq!(quote("hi"), "\"hi\"");
        assert_eq!(quote("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(quote("é"), "\"\\u00e9\"");
        // Outside the BMP: a surrogate pair, because Java literals are UTF-16.
        assert_eq!(quote("\u{1F600}"), "\"\\ud83d\\ude00\"");
    }

    #[test]
    fn number_literals_stay_writable() {
        assert_eq!(double(4.0), "4.0D");
        assert_eq!(double(0.5), "0.5D");
        assert_eq!(double(f64::NAN), "Double.NaN");
        assert_eq!(double(f64::NEG_INFINITY), "Double.NEGATIVE_INFINITY");
        assert_eq!(int(3.7), "3");
        assert_eq!(int(-3.7), "-3");
        assert_eq!(int(f64::INFINITY), "0");
        assert_eq!(int(1e30), "2147483647");
        assert_eq!(float(1.5), "1.5F");
    }

    #[test]
    fn comments_wrap_and_keep_paragraphs() {
        let mut source = Source::new();
        source.comment("one two three\n\nfour");
        assert_eq!(source.finish(), "// one two three\n//\n// four\n");
    }
}
