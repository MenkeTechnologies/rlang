//! Interactive REPL for `Rscript` — a `reedline` line editor over a persistent
//! host, so bindings and functions carry across prompts.
//!
//! Layout per turn:
//!
//! ```text
//! ─( HH:MM:SS )──< command N >──────────────────────────────{ rlang 0.1.0 }─
//! r❯ <buffer>
//! ```
//!
//! Tab completes from the R keyword set, the primitive library, and the names
//! bound in the live global environment, so a function defined on one prompt
//! completes on the next. History lives in `~/.rlang/history`.

use std::borrow::Cow;
use std::time::SystemTime;

use nu_ansi_term::{Color, Style};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, Completer, Emacs, FileBackedHistory, KeyCode,
    KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus, Reedline, ReedlineEvent, ReedlineMenu, Signal, Span, Suggestion,
};

use crate::{banner, builtins, host};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// R's reserved words.
const KEYWORDS: &[&str] = &[
    "if", "else", "for", "while", "repeat", "function", "break", "next", "in", "TRUE", "FALSE",
    "NULL", "NA", "Inf", "NaN",
];

/// Run the interactive loop until EOF (Ctrl-D).
pub fn run() {
    banner::print_banner();
    host::reset_host();

    let history = dirs::home_dir()
        .map(|h| h.join(".rlang").join("history"))
        .and_then(|p| {
            let _ = std::fs::create_dir_all(p.parent()?);
            FileBackedHistory::with_file(2000, p).ok()
        });

    let mut keys = default_emacs_keybindings();
    keys.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completions".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );

    let menu = ColumnarMenu::default().with_name("completions");
    let mut editor = Reedline::create()
        .with_completer(Box::new(RCompleter))
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)))
        .with_edit_mode(Box::new(Emacs::new(keys)));
    if let Some(h) = history {
        editor = editor.with_history(Box::new(h));
    }

    let mut n = 1usize;
    loop {
        let prompt = RPrompt { count: n };
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                n += 1;
                eval_line(&line);
            }
            Ok(Signal::CtrlC) => continue,
            _ => break,
        }
    }
}

/// Compile and run one line on the persistent host, echoing a visible result.
fn eval_line(line: &str) {
    let prog = match crate::compile(line) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", Color::Red.paint(format!("Error: {e}")));
            return;
        }
    };
    // Closure bodies from earlier prompts stay live, so append rather than
    // replace, and shift this program's closure ids by the existing count.
    let base = host::with_host(|h| h.closures.len());
    let shifted = crate::compiler::shift_closure_ids(prog, base);
    host::with_host(|h| h.closures.extend(shifted.closures));
    match host::run_main(shifted.main) {
        Ok(_) => {}
        Err(e) => eprintln!("{}", Color::Red.paint(format!("Error: {e}"))),
    }
}

struct RPrompt {
    count: usize,
}

impl Prompt for RPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let clock = clock();
        let head = format!("─( {clock} )──< command {} >", self.count);
        let tail = format!("{{ rlang {VERSION} }}─");
        let width = terminal_width();
        let fill = width.saturating_sub(head.chars().count() + tail.chars().count());
        let dim = Style::new().fg(Color::DarkGray);
        Cow::Owned(format!(
            "{}{}{}\n{} ",
            dim.paint(&head),
            dim.paint("─".repeat(fill)),
            dim.paint(&tail),
            Color::Cyan.bold().paint("r❯")
        ))
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("+ ")
    }
    fn render_prompt_history_search_indicator(&self, h: PromptHistorySearch) -> Cow<'_, str> {
        let prefix = match h.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!("({prefix}reverse-search: {}) ", h.term))
    }
}

fn clock() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = secs % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(80)
}

/// Completions: keywords, primitives, and whatever is bound right now.
struct RCompleter;

impl Completer for RCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let head = &line[..pos.min(line.len())];
        let start = head
            .rfind(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &head[start..];
        let mut names: Vec<String> = KEYWORDS.iter().map(|s| s.to_string()).collect();
        names.extend(builtins::PRIMITIVES.iter().map(|s| s.to_string()));
        names.extend(host::with_host(|h| {
            h.global.borrow().vars.keys().cloned().collect::<Vec<_>>()
        }));
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter(|n| n.starts_with(word))
            .map(|n| Suggestion {
                value: n,
                description: None,
                style: None,
                extra: None,
                span: Span::new(start, pos),
                append_whitespace: false,
                ..Default::default()
            })
            .collect()
    }
}
