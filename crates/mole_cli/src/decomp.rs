use serde_json::{json, Value};
use std::{
    cmp::Reverse,
    fs, io,
    path::{Component, Path, PathBuf},
};

use crate::{base_report, DecompCommand, DecompSearchOptions, DecompShowOptions, SCHEMA_VERSION};

const MAX_LIMIT: usize = 100;
const MAX_CONTEXT: usize = 80;
const ALLOWED_EXTENSIONS: &[&str] = &[
    "c", "h", "cpp", "hpp", "s", "S", "rs", "py", "md", "txt", "yml", "yaml",
];

pub(crate) fn decomp_report(root: &Path, command: &DecompCommand) -> Value {
    match command {
        DecompCommand::Search(options) => decomp_search_report(root, options, false),
        DecompCommand::Symbol(options) => decomp_search_report(root, options, true),
        DecompCommand::Show(options) => decomp_show_report(root, options),
    }
}

fn decomp_search_report(root: &Path, options: &DecompSearchOptions, symbol_mode: bool) -> Value {
    let command_name = if symbol_mode {
        "decomp symbol"
    } else {
        "decomp search"
    };
    let mut report = base_report(command_name, root);
    report["query"] = json!(options.query);
    report["limit"] = json!(options.limit.min(MAX_LIMIT).max(1));

    let decomp_root = resolve_decomp_root(root, options.decomp_root.as_deref());
    report["decomp_root"] = json!(decomp_root.display().to_string());

    if !decomp_root.is_dir() {
        report["ok"] = json!(false);
        report["matches"] = json!([]);
        report["errors"] = json!([format!(
            "decompiled Melee root not found: {}",
            decomp_root.display()
        )]);
        return report;
    }

    match search_decomp(&decomp_root, &options.query, options.limit, symbol_mode) {
        Ok(matches) => {
            report["ok"] = json!(true);
            report["match_count"] = json!(matches.len());
            report["matches"] = json!(matches);
            report["errors"] = json!([]);
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["match_count"] = json!(0);
            report["matches"] = json!([]);
            report["errors"] = json!([error]);
        }
    }

    report
}

fn decomp_show_report(root: &Path, options: &DecompShowOptions) -> Value {
    let mut report = base_report("decomp show", root);
    let decomp_root = resolve_decomp_root(root, options.decomp_root.as_deref());
    report["decomp_root"] = json!(decomp_root.display().to_string());
    report["path"] = json!(options.path);
    report["line"] = json!(options.line);
    report["context"] = json!(options.context.min(MAX_CONTEXT));

    if !decomp_root.is_dir() {
        report["ok"] = json!(false);
        report["excerpt"] = Value::Null;
        report["errors"] = json!([format!(
            "decompiled Melee root not found: {}",
            decomp_root.display()
        )]);
        return report;
    }

    match show_decomp_excerpt(&decomp_root, options) {
        Ok(excerpt) => {
            report["ok"] = json!(true);
            report["excerpt"] = excerpt;
            report["errors"] = json!([]);
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["excerpt"] = Value::Null;
            report["errors"] = json!([error]);
        }
    }

    report
}

fn resolve_decomp_root(root: &Path, override_root: Option<&str>) -> PathBuf {
    if let Some(path) = override_root {
        return PathBuf::from(path);
    }

    let local_research = root.join(".research").join("doldecomp-melee");
    if local_research.is_dir() {
        return local_research;
    }

    root.parent()
        .map(|parent| parent.join(".research").join("doldecomp-melee"))
        .unwrap_or(local_research)
}

fn search_decomp(
    decomp_root: &Path,
    query: &str,
    limit: usize,
    symbol_mode: bool,
) -> Result<Vec<Value>, String> {
    let limit = limit.min(MAX_LIMIT).max(1);
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();
    let mut files = Vec::new();
    collect_search_files(decomp_root, decomp_root, &mut files)
        .map_err(|error| format!("failed to scan decomp root: {error}"))?;
    files.sort();

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = relative_slash_path(decomp_root, &path);
        for (line_index, line) in text.lines().enumerate() {
            if !line.to_lowercase().contains(&query_lower) {
                continue;
            }
            let line_number = line_index + 1;
            let rank_reason = if symbol_mode && looks_like_exact_definition(line, query) {
                "exact_definition"
            } else if symbol_mode {
                "symbol_reference"
            } else {
                "text_match"
            };
            let rank = match rank_reason {
                "exact_definition" => 2,
                "symbol_reference" => 1,
                _ => 0,
            };
            matches.push((
                rank,
                relative.clone(),
                line_number,
                json!({
                    "path": relative,
                    "line": line_number,
                    "preview": line.trim(),
                    "rank_reason": rank_reason,
                    "suggested_command": format!(
                        "cargo run -p mole_cli -- decomp show \"{}\" --line {} --context 24 --json",
                        relative, line_number
                    ),
                }),
            ));
        }
    }

    matches.sort_by_key(|(rank, path, line, _)| (Reverse(*rank), path.clone(), *line));
    Ok(matches
        .into_iter()
        .take(limit)
        .map(|(_, _, _, value)| value)
        .collect())
}

fn collect_search_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if is_skipped_dir(&path) {
                continue;
            }
            collect_search_files(root, &path, files)?;
        } else if file_type.is_file() && is_searchable_file(root, &path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_skipped_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "build" | "target" | "__pycache__")
    )
}

fn is_searchable_file(root: &Path, path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() > 512 * 1024 {
        return false;
    }
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    path.starts_with(root) && ALLOWED_EXTENSIONS.contains(&extension)
}

fn looks_like_exact_definition(line: &str, symbol: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.ends_with(';') {
        return false;
    }
    let Some(paren_index) = trimmed.find('(') else {
        return false;
    };
    let identifier = trimmed[..paren_index]
        .chars()
        .rev()
        .take_while(|ch| is_identifier_char(*ch))
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if identifier != symbol {
        return false;
    }

    let prefix = trimmed[..paren_index]
        .strip_suffix(symbol)
        .unwrap_or_default()
        .trim();
    !prefix.is_empty()
        && prefix
            .chars()
            .all(|ch| is_identifier_char(ch) || ch.is_ascii_whitespace() || ch == '*')
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn show_decomp_excerpt(decomp_root: &Path, options: &DecompShowOptions) -> Result<Value, String> {
    let relative = safe_relative_path(&options.path)?;
    let path = decomp_root.join(&relative);
    if !path.is_file() {
        return Err(format!("decomp file not found: {}", relative.display()));
    }
    if !is_searchable_file(decomp_root, &path) {
        return Err(format!(
            "decomp file is not a supported text file: {}",
            relative.display()
        ));
    }

    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", relative.display()))?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(json!({
            "path": slash_path(&relative),
            "start_line": 0,
            "end_line": 0,
            "lines": [],
        }));
    }

    let context = options.context.min(MAX_CONTEXT);
    let requested_line = options.line.min(lines.len()).max(1);
    let start_line = requested_line.saturating_sub(context).max(1);
    let end_line = (requested_line + context).min(lines.len());
    let excerpt_lines = (start_line..=end_line)
        .map(|line_number| {
            json!({
                "line": line_number,
                "text": lines[line_number - 1],
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "path": slash_path(&relative),
        "start_line": start_line,
        "end_line": end_line,
        "lines": excerpt_lines,
    }))
}

fn safe_relative_path(path: &str) -> Result<PathBuf, String> {
    let relative = PathBuf::from(path);
    if relative.is_absolute() {
        return Err("decomp show path must be relative to the decomp root".to_string());
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("decomp show path may not escape the decomp root".to_string());
    }
    Ok(relative)
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(slash_path)
        .unwrap_or_else(|_| path.display().to_string())
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[allow(dead_code)]
const _: u8 = SCHEMA_VERSION;
