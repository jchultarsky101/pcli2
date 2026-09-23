//! Every example shown in `--help` is a command line the binary accepts.
//!
//! Examples are the part of the help people copy. One that names a renamed flag
//! teaches the wrong thing and fails on first use, so each is parsed here against
//! the real command tree.

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split a command line on whitespace, keeping "double quoted" words together.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn collect(command: &clap::Command, examples: &mut Vec<String>) {
    for help in [command.get_after_help(), command.get_after_long_help()]
        .into_iter()
        .flatten()
    {
        for line in strip_ansi(&help.to_string()).lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("pcli2 ") {
                // Drop a trailing "# comment".
                let rest = rest.split("  #").next().unwrap_or(rest);
                examples.push(format!("pcli2 {}", rest.trim()));
            }
        }
    }
    for sub in command.get_subcommands() {
        collect(sub, examples);
    }
}

#[test]
fn every_help_example_parses() {
    let command = pcli2::commands::create_full_command();
    let mut examples = Vec::new();
    collect(&command, &mut examples);
    assert!(
        examples.len() >= 20,
        "found only {} examples",
        examples.len()
    );
    for example in &examples {
        let result = pcli2::commands::create_full_command().try_get_matches_from(words(example));
        assert!(result.is_ok(), "{example}: {}", result.unwrap_err());
    }
}
