//! Substitute command (`:s/pattern/replacement/flags`).
//!
//! Implements vim-style search and replace on buffer text:
//! - `:s/pat/rep/` — replace first match on current line
//! - `:s/pat/rep/g` — replace all matches on current line
//! - `:%s/pat/rep/g` — replace all matches in entire buffer
//! - `:s/pat/rep/i` — case-insensitive matching
//! - `:s/pat/rep/n` — count matches without replacing
//!
//! The `c` (confirm) flag is deferred to a follow-up issue.

use {
    regex::Regex,
    reovim_domain_text::Position,
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{BufferApi, ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

/// Module ID for commands module.
const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

// =============================================================================
// Substitute Spec
// =============================================================================

/// Parsed substitute specification.
#[derive(Debug, Clone)]
struct SubstituteSpec {
    /// Search pattern (regex).
    pattern: String,
    /// Replacement string.
    replacement: String,
    /// Replace all matches on each line (not just the first).
    global: bool,
    /// Case-insensitive matching.
    case_insensitive: bool,
    /// Count matches only, don't replace.
    count_only: bool,
}

/// Parse the substitute syntax from raw arguments.
///
/// Expected input format: `/pattern/replacement/[flags]`
/// The first character determines the delimiter.
/// Escaped delimiters within pattern/replacement are handled.
///
/// Returns `None` if the input cannot be parsed.
#[cfg_attr(coverage_nightly, coverage(off))]
fn parse_substitute(input: &str) -> Option<SubstituteSpec> {
    if input.is_empty() {
        return None;
    }

    let mut chars = input.chars();
    let delimiter = chars.next()?;

    // Delimiter must not be alphanumeric or backslash
    if delimiter.is_alphanumeric() || delimiter == '\\' {
        return None;
    }

    let rest: String = chars.collect();
    let parts = split_by_delimiter(&rest, delimiter);

    // Need at least pattern and replacement (2 parts)
    if parts.len() < 2 {
        return None;
    }

    let pattern = parts[0].clone();
    let replacement = parts[1].clone();

    if pattern.is_empty() {
        return None;
    }

    // Parse flags from the third part (if present)
    let flags_str = parts.get(2).map_or("", String::as_str);
    let mut global = false;
    let mut case_insensitive = false;
    let mut count_only = false;

    for flag in flags_str.chars() {
        match flag {
            'g' => global = true,
            'i' => case_insensitive = true,
            'n' => count_only = true,
            _ => {} // Ignore unknown flags
        }
    }

    Some(SubstituteSpec {
        pattern,
        replacement,
        global,
        case_insensitive,
        count_only,
    })
}

/// Split a string by a delimiter, handling backslash escapes.
#[cfg_attr(coverage_nightly, coverage(off))]
fn split_by_delimiter(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            // Escaped delimiter: include literally without backslash.
            // Other escaped chars: keep the backslash + char.
            if ch != delimiter {
                current.push('\\');
            }
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == delimiter {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }

    // Handle trailing backslash
    if escaped {
        current.push('\\');
    }

    // Always include the last segment (flags or empty)
    parts.push(current);
    parts
}

// =============================================================================
// Substitute Command
// =============================================================================

/// Substitute command — `:s/pattern/replacement/flags`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubstituteCommand;

impl Command for SubstituteCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "substitute")
    }

    fn description(&self) -> &'static str {
        "Search and replace text"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "args",
            ArgKind::Rest,
            "Pattern, replacement, and flags (/pat/rep/flags)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["s", "substitute"]
    }
}

impl CommandHandler for SubstituteCommand {
    #[allow(clippy::too_many_lines)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get the raw substitute specification
        let raw = args.string("args").unwrap_or("");
        let Some(spec) = parse_substitute(raw) else {
            return CommandResult::error("E486: Invalid substitute syntax");
        };

        // Build regex
        let regex_pattern = if spec.case_insensitive {
            format!("(?i){}", spec.pattern)
        } else {
            spec.pattern.clone()
        };

        let regex = match Regex::new(&regex_pattern) {
            Ok(r) => r,
            Err(e) => {
                return CommandResult::error(&format!("E486: Invalid pattern: {e}"));
            }
        };

        // Determine line range
        let (start_line, end_line) = if let Some((s, e)) = args.range() {
            (s, e)
        } else {
            // Default: current line only
            let cursor_line = runtime.windows().active().map_or(0, |w| w.cursor.line);
            (cursor_line, cursor_line)
        };

        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);
        let end_line = end_line.min(line_count.saturating_sub(1));

        if start_line > end_line || line_count == 0 {
            return CommandResult::error("E486: Pattern not found");
        }

        // Count-only mode
        if spec.count_only {
            let mut total_matches = 0usize;
            let mut lines_with_matches = 0usize;

            for line_idx in start_line..=end_line {
                if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                    let count = regex.find_iter(&line).count();
                    if count > 0 {
                        total_matches += count;
                        lines_with_matches += 1;
                    }
                }
            }

            tracing::info!(
                "{total_matches} match{} on {lines_with_matches} line{}",
                if total_matches == 1 { "" } else { "es" },
                if lines_with_matches == 1 { "" } else { "s" },
            );
            return CommandResult::Success;
        }

        // Perform substitutions
        let mut total_subs = 0usize;
        let mut lines_changed = 0usize;
        let mut last_sub_line = start_line;

        // Process lines in reverse order to preserve positions
        for line_idx in (start_line..=end_line).rev() {
            let Some(line) = runtime.buffer_line(buffer_id, line_idx) else {
                continue;
            };

            let new_line = if spec.global {
                regex
                    .replace_all(&line, spec.replacement.as_str())
                    .into_owned()
            } else {
                regex.replace(&line, spec.replacement.as_str()).into_owned()
            };

            if new_line != line {
                let line_len = line.len();
                let start = Position::new(line_idx, 0);
                let end = Position::new(line_idx, line_len);
                runtime.delete_range(buffer_id, start, end);
                runtime.insert_text(buffer_id, start, &new_line);

                let subs_on_line = if spec.global {
                    regex.find_iter(&line).count()
                } else {
                    1
                };
                total_subs += subs_on_line;
                lines_changed += 1;
                // Track the earliest changed line (we're iterating in reverse)
                last_sub_line = line_idx;
            }
        }

        if total_subs == 0 {
            return CommandResult::error("E486: Pattern not found");
        }

        // Move cursor to the last substitution line
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = last_sub_line;
            window.cursor.column = 0;
        }

        runtime.record_cursor_move(buffer_id);

        tracing::info!(
            "{total_subs} substitution{} on {lines_changed} line{}",
            if total_subs == 1 { "" } else { "s" },
            if lines_changed == 1 { "" } else { "s" },
        );

        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "substitute_tests.rs"]
mod tests;
