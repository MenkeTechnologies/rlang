//! Tests for `eval_captured`, the embedder entry point: variables are seeded
//! after the host reset that starts every run, so a program can be handed input
//! without splicing an escaped literal into its source.

/// A seeded variable is an ordinary character vector the program can use.
#[test]
fn seeded_vars_survive_the_host_reset() {
    let (result, out) = rlang::eval_captured("cat(toupper(stdin))", &[("stdin", "hello")]);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(out, "HELLO");
}

/// It really is a character vector, not a bare handle that merely prints right.
#[test]
fn seeded_vars_are_character_vectors() {
    let (result, out) =
        rlang::eval_captured("cat(class(stdin), nchar(stdin))", &[("stdin", "abc")]);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(out, "character 3");
}

/// Text that would otherwise be read as syntax is data: nothing is escaped
/// anywhere, so quotes and backslashes round-trip byte for byte.
#[test]
fn hostile_input_is_data_not_syntax() {
    let hostile = "a \"quoted\" \\ back";
    let (result, out) = rlang::eval_captured("cat(stdin)", &[("stdin", hostile)]);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(out, hostile);
}

/// A failing run is distinguishable from one that printed the word "Error" —
/// the outcome comes back separately from the transcript.
#[test]
fn failure_is_separate_from_the_transcript() {
    let (result, out) = rlang::eval_captured("cat('before\n'); stop('boom')", &[]);
    assert!(result.is_err(), "expected the stop() to surface");
    assert_eq!(out, "before\n");
}
