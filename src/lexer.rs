//! The R lexer.
//!
//! Two R-specific rules shape it:
//!
//! * **Newlines are significant** — a newline ends an expression when the
//!   expression is complete. R suppresses that inside `(` and `[` but not
//!   inside `{`, so the lexer keeps a bracket stack and drops newlines while the
//!   innermost open bracket is a paren or a square bracket.
//! * **`[[` is one token, `]]` is never one.** Lexing a closing `]]` would
//!   mis-tokenize `a[b[1]]`; instead `[[` opens and the parser consumes two
//!   separate `]` tokens to close it.

use crate::ast::NaKind;

/// A lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Int(i64),
    Str(String),
    Ident(String),
    /// `%%`, `%/%`, `%in%`, or any user-defined `%op%` (stored without the
    /// surrounding percent signs).
    Special(String),

    // keywords
    If,
    Else,
    For,
    While,
    Repeat,
    Function,
    Break,
    Next,
    In,
    True,
    False,
    Null,
    Na(NaKind),
    Inf,
    NaN,
    Dots,

    // operators
    Assign,      // <-
    SuperAssign, // <<-
    RightAssign, // ->
    RightSuper,  // ->>
    Eq,          // =
    EqEq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    /// `|>` — the native forward pipe.
    PipeGt,
    Colon,
    /// `::` and `:::` — namespace access (both lex to this token).
    ColonColon,
    Tilde,
    Question,
    Dollar,
    At,
    Comma,
    Semi,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    /// `[[`
    LBracket2,
    RBracket,

    Newline,
    Eof,
}

/// A token plus the source line it came from, and whether whitespace preceded
/// it (unused today, kept parallel to the sibling lexers).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
}

/// What kind of bracket is currently open (decides newline suppression).
#[derive(PartialEq)]
enum Bracket {
    Paren,
    Square,
    Brace,
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    brackets: Vec<Bracket>,
    out: Vec<Token>,
}

/// Tokenize an R source string.
pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut lx = Lexer {
        src: src.as_bytes(),
        pos: 0,
        line: 1,
        brackets: Vec::new(),
        out: Vec::new(),
    };
    lx.run()?;
    lx.out.push(Token {
        tok: Tok::Eof,
        line: lx.line,
    });
    Ok(lx.out)
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }
    fn at(&self, off: usize) -> u8 {
        *self.src.get(self.pos + off).unwrap_or(&0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        c
    }
    fn push(&mut self, tok: Tok) {
        let line = self.line;
        self.out.push(Token { tok, line });
    }
    /// Newlines are dropped while the innermost open bracket is `(` or `[`.
    fn newline_significant(&self) -> bool {
        !matches!(
            self.brackets.last(),
            Some(Bracket::Paren) | Some(Bracket::Square)
        )
    }

    fn run(&mut self) -> Result<(), String> {
        while self.pos < self.src.len() {
            let c = self.peek();
            match c {
                b' ' | b'\t' | b'\r' => {
                    self.pos += 1;
                }
                b'\n' => {
                    self.pos += 1;
                    if self.newline_significant() {
                        self.push(Tok::Newline);
                    }
                    self.line += 1;
                }
                b'#' => {
                    while self.pos < self.src.len() && self.peek() != b'\n' {
                        self.pos += 1;
                    }
                }
                b'"' | b'\'' => self.string(c)?,
                b'`' => self.backtick_ident()?,
                b'%' => self.special()?,
                b'0'..=b'9' => self.number()?,
                b'.' if self.at(1).is_ascii_digit() => self.number()?,
                _ if is_ident_start(c) => self.ident(),
                _ => self.operator()?,
            }
        }
        Ok(())
    }

    /// A string literal is scanned as BYTES, because `\xNN` and `\NNN` name a
    /// byte rather than a character: R's `"\xc3\xa9"` is one `é`, which only
    /// falls out if the two escapes land next to each other in the buffer.
    fn string(&mut self, quote: u8) -> Result<(), String> {
        self.pos += 1;
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            if self.pos >= self.src.len() {
                return Err(format!("line {}: unterminated string", self.line));
            }
            let c = self.bump();
            match c {
                b'\\' => self.escape(&mut bytes)?,
                _ if c == quote => break,
                b'\n' => {
                    self.line += 1;
                    bytes.push(b'\n');
                }
                // Multi-byte UTF-8 passes through byte by byte; the buffer is
                // validated once, at the end.
                _ => bytes.push(c),
            }
        }
        let s = String::from_utf8(bytes)
            .map_err(|_| format!("line {}: invalid UTF-8 in string literal", self.line))?;
        self.push(Tok::Str(s));
        Ok(())
    }

    /// One backslash escape, appended to `out`.
    ///
    /// R's escape set is closed — anything outside it is a parse error, not a
    /// literal character — and each numeric form has its own digit budget:
    /// `\NNN` is up to three octal digits, `\xNN` up to two hex, `\uXXXX` up to
    /// four (capped at `U+FFFF`), `\UXXXXXXXX` up to eight. Reading a digit past
    /// the budget silently swallows the next character of the string, which is
    /// how `"　b"` used to lex as `U+3000B` instead of `U+3000` then `b`.
    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let e = self.bump();
        let byte = match e {
            b'a' => 0x07,
            b'b' => 0x08,
            b'f' => 0x0C,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 0x0B,
            b'\\' | b'"' | b'\'' | b'`' | b' ' => e,
            b'0'..=b'7' => {
                // Octal, this digit included, at most three, at most \377.
                let mut v = u32::from(e - b'0');
                for _ in 0..2 {
                    match self.peek() {
                        d @ b'0'..=b'7' => {
                            v = v * 8 + u32::from(d - b'0');
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                if v > 0o377 {
                    return Err(format!(
                        "line {}: \\{v:o} exceeds maximum allowed octal value \\377",
                        self.line
                    ));
                }
                v as u8
            }
            b'x' => {
                let v = self.hex_digits(2);
                let Some(v) = v else {
                    return Err(format!(
                        "line {}: '\\x' used without hex digits in character string",
                        self.line
                    ));
                };
                v as u8
            }
            b'u' | b'U' => {
                let (max_digits, max_val) = if e == b'u' {
                    (4, 0xFFFF)
                } else {
                    (8, 0x10FFFF)
                };
                let braced = self.peek() == b'{';
                if braced {
                    self.pos += 1;
                }
                let v = self.hex_digits(if braced { 8 } else { max_digits });
                if braced && self.peek() == b'}' {
                    self.pos += 1;
                }
                let Some(v) = v else {
                    return Err(format!(
                        "line {}: '\\{}' used without hex digits in character string",
                        self.line, e as char
                    ));
                };
                if v > max_val {
                    return Err(format!(
                        "line {}: invalid \\{}{{xxxx}} sequence",
                        self.line, e as char
                    ));
                }
                if v == 0 {
                    return Err(format!("line {}: nul character not allowed", self.line));
                }
                let c = char::from_u32(v).ok_or_else(|| {
                    format!("line {}: invalid \\{} value {v:x}", self.line, e as char)
                })?;
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                return Ok(());
            }
            other => {
                return Err(format!(
                    "line {}: '\\{}' is an unrecognized escape in character string",
                    self.line, other as char
                ))
            }
        };
        // A nul from an octal or hex escape is dropped rather than stored: R's
        // strings are C strings, so `"a\0b"` is "ab".
        if byte != 0 {
            out.push(byte);
        }
        Ok(())
    }

    /// Up to `max` hex digits as a value, or `None` if there is not even one.
    fn hex_digits(&mut self, max: usize) -> Option<u32> {
        let mut v: u32 = 0;
        let mut n = 0;
        while n < max && self.peek().is_ascii_hexdigit() {
            v = v * 16 + char::from(self.bump()).to_digit(16)?;
            n += 1;
        }
        (n > 0).then_some(v)
    }

    fn backtick_ident(&mut self) -> Result<(), String> {
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.src.len() && self.peek() != b'`' {
            self.pos += 1;
        }
        if self.pos >= self.src.len() {
            return Err(format!("line {}: unterminated backtick name", self.line));
        }
        let name = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        self.pos += 1;
        self.push(Tok::Ident(name));
        Ok(())
    }

    /// `%...%` — the special infix form. `%%` and `%/%` are just the built-in
    /// members of that family.
    fn special(&mut self) -> Result<(), String> {
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.src.len() && self.peek() != b'%' {
            if self.peek() == b'\n' {
                return Err(format!("line {}: unterminated %operator%", self.line));
            }
            self.pos += 1;
        }
        if self.pos >= self.src.len() {
            return Err(format!("line {}: unterminated %operator%", self.line));
        }
        let name = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        self.pos += 1;
        self.push(Tok::Special(name));
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        let start = self.pos;
        if self.peek() == b'0' && (self.at(1) | 0x20) == b'x' {
            self.pos += 2;
            while self.peek().is_ascii_hexdigit() {
                self.pos += 1;
            }
            let text = std::str::from_utf8(&self.src[start + 2..self.pos]).unwrap_or("0");
            let n = i64::from_str_radix(text, 16)
                .map_err(|e| format!("line {}: bad hex literal: {e}", self.line))?;
            if self.peek() == b'L' {
                self.pos += 1;
                self.push(Tok::Int(n));
            } else {
                self.push(Tok::Num(n as f64));
            }
            return Ok(());
        }
        while self.peek().is_ascii_digit() {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.peek() == b'.' {
            is_float = true;
            self.pos += 1;
            while self.peek().is_ascii_digit() {
                self.pos += 1;
            }
        }
        if (self.peek() | 0x20) == b'e' {
            let save = self.pos;
            self.pos += 1;
            if self.peek() == b'+' || self.peek() == b'-' {
                self.pos += 1;
            }
            if self.peek().is_ascii_digit() {
                is_float = true;
                while self.peek().is_ascii_digit() {
                    self.pos += 1;
                }
            } else {
                self.pos = save;
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let v: f64 = text
            .parse()
            .map_err(|e| format!("line {}: bad number '{text}': {e}", self.line))?;
        // `1L` is an integer literal; everything else — including a bare `1` — is
        // a double, which is R's rule and the source of `1 == 1L` being TRUE
        // while `identical(1, 1L)` is FALSE.
        if self.peek() == b'L' && !is_float {
            self.pos += 1;
            self.push(Tok::Int(v as i64));
        } else {
            if self.peek() == b'L' {
                self.pos += 1;
            }
            self.push(Tok::Num(v));
        }
        Ok(())
    }

    fn ident(&mut self) {
        let start = self.pos;
        while self.pos < self.src.len() && is_ident_part(self.peek()) {
            self.pos += 1;
        }
        let word = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        let tok = match word.as_str() {
            "if" => Tok::If,
            "else" => Tok::Else,
            "for" => Tok::For,
            "while" => Tok::While,
            "repeat" => Tok::Repeat,
            "function" => Tok::Function,
            "break" => Tok::Break,
            "next" => Tok::Next,
            "in" => Tok::In,
            "TRUE" | "T" => Tok::True,
            "FALSE" | "F" => Tok::False,
            "NULL" => Tok::Null,
            "NA" => Tok::Na(NaKind::Logical),
            "NA_integer_" => Tok::Na(NaKind::Integer),
            "NA_real_" => Tok::Na(NaKind::Real),
            "NA_character_" => Tok::Na(NaKind::Character),
            "Inf" => Tok::Inf,
            "NaN" => Tok::NaN,
            "..." => Tok::Dots,
            _ => Tok::Ident(word),
        };
        self.push(tok);
    }

    fn operator(&mut self) -> Result<(), String> {
        let c = self.bump();
        let tok = match c {
            b'<' => match (self.peek(), self.at(1)) {
                (b'<', b'-') => {
                    self.pos += 2;
                    Tok::SuperAssign
                }
                (b'-', _) => {
                    self.pos += 1;
                    Tok::Assign
                }
                (b'=', _) => {
                    self.pos += 1;
                    Tok::Le
                }
                _ => Tok::Lt,
            },
            b'-' => {
                if self.peek() == b'>' {
                    self.pos += 1;
                    if self.peek() == b'>' {
                        self.pos += 1;
                        Tok::RightSuper
                    } else {
                        Tok::RightAssign
                    }
                } else {
                    Tok::Minus
                }
            }
            b'>' => {
                if self.peek() == b'=' {
                    self.pos += 1;
                    Tok::Ge
                } else {
                    Tok::Gt
                }
            }
            b'=' => {
                if self.peek() == b'=' {
                    self.pos += 1;
                    Tok::EqEq
                } else {
                    Tok::Eq
                }
            }
            b'!' => {
                if self.peek() == b'=' {
                    self.pos += 1;
                    Tok::Ne
                } else {
                    Tok::Bang
                }
            }
            b'&' => {
                if self.peek() == b'&' {
                    self.pos += 1;
                    Tok::AmpAmp
                } else {
                    Tok::Amp
                }
            }
            b'|' => match self.peek() {
                b'|' => {
                    self.pos += 1;
                    Tok::PipePipe
                }
                b'>' => {
                    self.pos += 1;
                    Tok::PipeGt
                }
                _ => Tok::Pipe,
            },
            b':' => {
                if self.peek() == b':' {
                    self.pos += 1;
                    if self.peek() == b':' {
                        self.pos += 1;
                    }
                    Tok::ColonColon
                } else {
                    Tok::Colon
                }
            }
            b'+' => Tok::Plus,
            b'*' => Tok::Star,
            b'/' => Tok::Slash,
            b'^' => Tok::Caret,
            b'~' => Tok::Tilde,
            b'?' => Tok::Question,
            b'$' => Tok::Dollar,
            b'@' => Tok::At,
            b',' => Tok::Comma,
            b';' => Tok::Semi,
            b'(' => {
                self.brackets.push(Bracket::Paren);
                Tok::LParen
            }
            b')' => {
                self.brackets.pop();
                Tok::RParen
            }
            b'{' => {
                self.brackets.push(Bracket::Brace);
                Tok::LBrace
            }
            b'}' => {
                self.brackets.pop();
                Tok::RBrace
            }
            b'[' => {
                self.brackets.push(Bracket::Square);
                if self.peek() == b'[' {
                    self.pos += 1;
                    self.brackets.push(Bracket::Square);
                    Tok::LBracket2
                } else {
                    Tok::LBracket
                }
            }
            b']' => {
                self.brackets.pop();
                Tok::RBracket
            }
            other => {
                return Err(format!(
                    "line {}: unexpected character '{}'",
                    self.line, other as char
                ))
            }
        };
        self.push(tok);
        Ok(())
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'.' || c == b'_' || c >= 0x80
}

fn is_ident_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn newlines_are_suppressed_inside_parens_only() {
        // Inside `(`: dropped. Inside `{`: kept.
        assert!(!toks("f(1,\n2)").contains(&Tok::Newline));
        assert!(toks("{1\n2}").contains(&Tok::Newline));
    }

    #[test]
    fn double_bracket_opens_but_never_closes_as_one_token() {
        // `a[b[1]]` must not produce a `]]`, else the inner index eats the outer.
        let t = toks("a[b[1]]");
        assert_eq!(t.iter().filter(|x| **x == Tok::RBracket).count(), 2);
        assert!(!t.contains(&Tok::LBracket2));
        assert!(toks("x[[1]]").contains(&Tok::LBracket2));
    }

    #[test]
    fn l_suffix_selects_integer_literals() {
        assert_eq!(toks("1")[0], Tok::Num(1.0));
        assert_eq!(toks("1L")[0], Tok::Int(1));
        assert_eq!(toks("0xffL")[0], Tok::Int(255));
        assert_eq!(toks("1e3")[0], Tok::Num(1000.0));
    }

    #[test]
    fn special_operators_strip_their_percents() {
        assert_eq!(toks("a %in% b")[1], Tok::Special("in".into()));
        assert_eq!(toks("a %% b")[1], Tok::Special("".into()));
        assert_eq!(toks("a %/% b")[1], Tok::Special("/".into()));
    }

    #[test]
    fn arrows_and_comparisons_do_not_collide() {
        assert_eq!(toks("x <- 1")[1], Tok::Assign);
        assert_eq!(toks("x <<- 1")[1], Tok::SuperAssign);
        assert_eq!(toks("1 -> x")[1], Tok::RightAssign);
        assert_eq!(toks("1 ->> x")[1], Tok::RightSuper);
        assert_eq!(toks("x <= 1")[1], Tok::Le);
        assert_eq!(toks("x < 1")[1], Tok::Lt);
    }

    #[test]
    fn backticks_make_arbitrary_names() {
        assert_eq!(toks("`my var`")[0], Tok::Ident("my var".into()));
    }
}
