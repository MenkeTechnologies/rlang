//! The regular-expression engine behind `grepl`, `sub`, `gsub`, `regexpr`,
//! `strsplit` and friends.
//!
//! R's default (TRE) and `perl = TRUE` (PCRE) patterns both allow back-references
//! (`(a)\1`), and PCRE allows look-around (`(?<=a)b`). The `regex` crate is
//! linear-time and has neither, so a pattern it rejects for exactly those
//! features is handed to `fancy-regex`, which backtracks. Everything `regex`
//! accepts stays on it, so the common case keeps its guarantees.

/// One compiled pattern, on whichever engine could take it.
pub enum Re {
    Std(regex::Regex),
    Fancy(Box<fancy_regex::Regex>),
}

/// A match: byte offsets into the subject, and the subject itself.
#[derive(Clone, Copy)]
pub struct Match<'t> {
    start: usize,
    end: usize,
    text: &'t str,
}

impl<'t> Match<'t> {
    pub fn start(&self) -> usize {
        self.start
    }
    pub fn end(&self) -> usize {
        self.end
    }
    pub fn as_str(&self) -> &'t str {
        &self.text[self.start..self.end]
    }
}

/// The groups of one match, group 0 first; a group that did not take part is
/// `None`.
pub struct Captures<'t> {
    groups: Vec<Option<(usize, usize)>>,
    text: &'t str,
}

impl<'t> Captures<'t> {
    pub fn get(&self, i: usize) -> Option<Match<'t>> {
        self.groups
            .get(i)
            .copied()
            .flatten()
            .map(|(start, end)| Match {
                start,
                end,
                text: self.text,
            })
    }
    pub fn len(&self) -> usize {
        self.groups.len()
    }
    /// A match always has group 0, so this is never true; it pairs with `len`.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl Re {
    /// Compile `pattern`. The `regex` crate goes first; only a rejection for
    /// look-around or back-references retries on the backtracking engine, and
    /// then the error reported for a pattern neither accepts is the first
    /// engine's, which is the one users meet most.
    pub fn new(pattern: &str) -> Result<Re, String> {
        match regex::Regex::new(pattern) {
            Ok(re) => Ok(Re::Std(re)),
            Err(e) => {
                let msg = e.to_string();
                let needs_backtracking =
                    msg.contains("look-around") || msg.contains("backreferences");
                if needs_backtracking {
                    if let Ok(re) = fancy_regex::Regex::new(pattern) {
                        return Ok(Re::Fancy(Box::new(re)));
                    }
                }
                Err(msg)
            }
        }
    }

    pub fn is_match(&self, text: &str) -> bool {
        match self {
            Re::Std(re) => re.is_match(text),
            Re::Fancy(re) => re.is_match(text).unwrap_or(false),
        }
    }

    pub fn find<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        match self {
            Re::Std(re) => re.find(text).map(|m| Match {
                start: m.start(),
                end: m.end(),
                text,
            }),
            Re::Fancy(re) => re.find(text).ok().flatten().map(|m| Match {
                start: m.start(),
                end: m.end(),
                text,
            }),
        }
    }

    pub fn find_iter<'t>(&self, text: &'t str) -> Vec<Match<'t>> {
        match self {
            Re::Std(re) => re
                .find_iter(text)
                .map(|m| Match {
                    start: m.start(),
                    end: m.end(),
                    text,
                })
                .collect(),
            Re::Fancy(re) => re
                .find_iter(text)
                .filter_map(Result::ok)
                .map(|m| Match {
                    start: m.start(),
                    end: m.end(),
                    text,
                })
                .collect(),
        }
    }

    pub fn captures<'t>(&self, text: &'t str) -> Option<Captures<'t>> {
        match self {
            Re::Std(re) => re.captures(text).map(|c| Captures {
                groups: c.iter().map(|g| g.map(|m| (m.start(), m.end()))).collect(),
                text,
            }),
            Re::Fancy(re) => re.captures(text).ok().flatten().map(|c| Captures {
                groups: c.iter().map(|g| g.map(|m| (m.start(), m.end()))).collect(),
                text,
            }),
        }
    }

    fn captures_all<'t>(&self, text: &'t str) -> Vec<Captures<'t>> {
        match self {
            Re::Std(re) => re
                .captures_iter(text)
                .map(|c| Captures {
                    groups: c.iter().map(|g| g.map(|m| (m.start(), m.end()))).collect(),
                    text,
                })
                .collect(),
            Re::Fancy(re) => re
                .captures_iter(text)
                .filter_map(Result::ok)
                .map(|c| Captures {
                    groups: c.iter().map(|g| g.map(|m| (m.start(), m.end()))).collect(),
                    text,
                })
                .collect(),
        }
    }

    /// `text` with the first match (or, with `all`, every match) replaced by
    /// what `with` makes of its captures.
    pub fn replace(&self, text: &str, all: bool, with: &dyn Fn(&Captures) -> String) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let hits = match all {
            true => self.captures_all(text),
            false => self.captures(text).into_iter().collect(),
        };
        for caps in hits {
            let Some(whole) = caps.get(0) else { continue };
            out.push_str(&text[last..whole.start()]);
            out.push_str(&with(&caps));
            last = whole.end();
        }
        out.push_str(&text[last..]);
        out
    }
}
