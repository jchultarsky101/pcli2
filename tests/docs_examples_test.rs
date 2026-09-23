//! Every `pcli2 ...` example in the README and the user guide is a command line
//! the binary accepts.
//!
//! The documentation is where people copy commands from. This parses each example
//! (it runs nothing) against the real command tree, so a renamed or removed flag
//! fails the build instead of the reader.

use std::path::{Path, PathBuf};

fn doc_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("README.md")];
    for entry in std::fs::read_dir(root.join("docs/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "md") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Shell words, honouring "double" and 'single' quotes; stops at an unquoted
/// comment, pipe or redirection, since only the pcli2 part is checked.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, '#') if !in_word => break,
            (None, '|') | (None, '>') | (None, '&') | (None, ';') => break,
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            (None, c) => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(current);
    }
    words
}

/// The pcli2 command lines inside the file's code blocks, with `\`
/// continuations joined and leading `VAR=value` assignments dropped.
fn examples(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut in_code = false;
    let mut shell = false;
    let mut pending: Option<(usize, String)> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(language) = line.strip_prefix("```") {
            in_code = !in_code;
            // PowerShell and other languages continue lines differently.
            shell = in_code
                && matches!(
                    language.trim(),
                    "" | "bash" | "sh" | "shell" | "zsh" | "console"
                );
            continue;
        }
        if !in_code || !shell {
            continue;
        }
        if let Some((start, mut joined)) = pending.take() {
            joined.push(' ');
            joined.push_str(line.trim_end_matches('\\'));
            if line.ends_with('\\') {
                pending = Some((start, joined));
            } else {
                found.push((start, joined));
            }
            continue;
        }
        let line = line.trim_start_matches("$ ");
        let mut rest = line;
        // Leading environment assignments: PCLI2_FORMAT=csv pcli2 ...
        while let Some((first, tail)) = rest.split_once(' ') {
            if first.contains('=') && !first.starts_with('-') {
                rest = tail.trim_start();
            } else {
                break;
            }
        }
        if !rest.starts_with("pcli2 ") {
            continue;
        }
        if rest.ends_with('\\') {
            pending = Some((index + 1, rest.trim_end_matches('\\').to_string()));
        } else {
            found.push((index + 1, rest.to_string()));
        }
    }
    found
}

#[test]
fn every_documented_example_parses() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for file in doc_files() {
        let text = std::fs::read_to_string(&file).unwrap();
        for (line, example) in examples(&text) {
            let args = words(&example);
            // Placeholders stand for a value the reader supplies, and an example
            // marked "# error" shows a refusal on purpose.
            if args
                .iter()
                .any(|a| a.contains('<') || a.contains("...") || a.contains('$'))
                || example.contains("# error")
            {
                continue;
            }
            checked += 1;
            let result = pcli2::commands::create_full_command().try_get_matches_from(&args);
            if let Err(error) = result {
                // --help and --version end parsing successfully.
                if matches!(
                    error.kind(),
                    clap::error::ErrorKind::DisplayHelp
                        | clap::error::ErrorKind::DisplayVersion
                        | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                ) {
                    continue;
                }
                // A command reference line ("pcli2 asset get   # Get asset
                // details") names a command without its arguments; that is a
                // synopsis, not an example.
                let synopsis = !args.iter().any(|a| a.starts_with('-'))
                    && matches!(
                        error.kind(),
                        clap::error::ErrorKind::MissingRequiredArgument
                            | clap::error::ErrorKind::MissingSubcommand
                    );
                if synopsis {
                    continue;
                }
                failures.push(format!(
                    "{}:{}: {}\n    {}",
                    file.file_name().unwrap().to_string_lossy(),
                    line,
                    example,
                    error.to_string().lines().next().unwrap_or_default()
                ));
            }
        }
    }
    assert!(checked > 100, "only {checked} examples found");
    assert!(
        failures.is_empty(),
        "{} example(s) do not parse:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Links to the documentation site point at pages that exist. The user guide is
/// published under `/pcli2/book/`; the site root only has the landing page
/// (the README), the changelog and the downloads, so a root-level
/// `/pcli2/<page>.html` link is a 404 (it was, for the whole README list).
#[test]
fn documentation_site_links_point_at_existing_pages() {
    const SITE: &str = "jchultarsky101.github.io/pcli2/";
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut broken = Vec::new();
    for file in doc_files() {
        let text = std::fs::read_to_string(&file).unwrap();
        for (at, _) in text.match_indices(SITE) {
            let rest = &text[at + SITE.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || "_-./".contains(c)))
                .unwrap_or(rest.len());
            let page = &rest[..end];
            let ok = match page {
                "" | "changelog/" | "artifacts/" | "book/" => true,
                _ => page
                    .strip_prefix("book/")
                    .and_then(|p| p.strip_suffix(".html"))
                    .is_some_and(|name| root.join("docs/src").join(format!("{name}.md")).is_file()),
            };
            if !ok {
                broken.push(format!("{}: {SITE}{page}", file.display()));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "links to missing pages:\n{}",
        broken.join("\n")
    );
}

/// The README is also the documentation site's landing page, where a link
/// relative to the repository (`CONTRIBUTING.md`) is a 404. Links in it are
/// absolute or in-page anchors.
#[test]
fn readme_links_work_on_github_and_on_the_site() {
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let relative: Vec<&str> = readme
        .match_indices("](")
        .map(|(at, _)| {
            let target = &readme[at + 2..];
            &target[..target.find(')').unwrap_or(target.len())]
        })
        .filter(|target| !target.starts_with("http") && !target.starts_with('#'))
        .collect();
    assert!(
        relative.is_empty(),
        "relative links in README.md: {relative:?}"
    );
}
