use crate::model::*;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Load a lab submission file, returning a list of steps.
///
/// Supports three formats:
/// 1. **New format (no blank lines)**: Sequential step number detection.
///    Step boundaries detected by matching the next expected step number.
/// 2. **Old format ($ prefix)**: Lines starting with `$ ` are commands.
///    Entries are flattened into a single Vec<String> per step.
/// 3. **Legacy format (blank line separated)**: Step blocks with step numbers.
///
/// Returns a single empty step if the file is empty or cannot be read.
pub fn load_file(path: &Path) -> Vec<Step> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![new_step()],
    };

    if content.trim().is_empty() {
        return vec![new_step()];
    }

    // Detect old format: any line starts with "$ "
    let is_old_format = content.lines().any(|l| l.starts_with("$ "));

    if is_old_format {
        load_old_format(&content)
    } else {
        load_new_format(&content)
    }
}

/// Parse the new format (no blank lines).
///
/// Sequential step number detection: track next_expected = 1, 2, 3, ...
/// When a line equals next_expected.to_string() (trimmed), start a new step.
/// This is robust against output containing bare numbers since we only match
/// the NEXT expected number.
///
/// Blank lines are skipped for backward compatibility with legacy format.
fn load_new_format(content: &str) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut current_step: Step = Vec::new();
    let mut next_expected = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip blank lines (for legacy format compatibility)
        if trimmed.is_empty() {
            continue;
        }

        // Check if this line is the next expected step number
        if trimmed == next_expected.to_string() {
            // Flush the current step if it has content
            if !current_step.is_empty() {
                steps.push(current_step.clone());
                current_step.clear();
            }
            next_expected += 1;
            continue;
        }

        // Not a step number - add to current step
        current_step.push(line.to_string());
    }

    // Flush the last step
    if !current_step.is_empty() {
        steps.push(current_step);
    }

    // If we found step markers but no content, or if the file started with step 1
    // but had no other content, handle the edge case
    if steps.is_empty() {
        vec![new_step()]
    } else {
        steps
    }
}

/// Parse the old `$ ` prefixed format.
///
/// Uses sequential step number detection (same as new format).
/// `$ cmd` lines store just `cmd` (strip `$ `).
/// Bare `$` lines and blank lines are skipped.
fn load_old_format(content: &str) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut current_step: Step = Vec::new();
    let mut next_expected: u32 = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip blank lines entirely
        if trimmed.is_empty() {
            continue;
        }

        // Check if this is the next expected step number
        if trimmed == next_expected.to_string() {
            if !current_step.is_empty() {
                steps.push(current_step.clone());
                current_step.clear();
            }
            next_expected += 1;
            continue;
        }

        // Bare "$" - drop it
        if line == "$" {
            continue;
        }

        // "$ cmd" - store just the command (strip "$ ")
        if line.starts_with("$ ") {
            current_step.push(line[2..].to_string());
            continue;
        }

        // Regular output line
        current_step.push(line.to_string());
    }

    // Flush remaining
    if !current_step.is_empty() {
        steps.push(current_step);
    }

    if steps.is_empty() {
        vec![new_step()]
    } else {
        steps
    }
}

/// Save steps to a lab submission file in the clean format.
///
/// Format: ZERO blank lines. Step number on its own line, then content lines,
/// then next step number. File ends with exactly one newline.
///
/// Example:
/// ```
/// 1
/// {giant:~} echo hello
/// hello
/// 2
/// {giant:~} ls
/// file.txt
/// ```
pub fn save_file(path: &Path, steps: &[Step]) -> io::Result<()> {
    let mut output = String::new();

    for (i, step) in steps.iter().enumerate() {
        // Step number (1-indexed)
        output.push_str(&(i + 1).to_string());
        output.push('\n');

        // All lines of the step
        for line in step {
            output.push_str(line);
            output.push('\n');
        }
    }

    // Strip trailing blank lines, then ensure exactly one trailing newline
    let trimmed = output.trim_end_matches('\n');
    let final_output = if trimmed.is_empty() {
        String::from("\n")
    } else {
        format!("{}\n", trimmed)
    };

    let mut file = fs::File::create(path)?;
    file.write_all(final_output.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn load_empty_file() {
        let f = tmp_file("");
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0], vec![String::new()]);
    }

    #[test]
    fn load_nonexistent_file() {
        let steps = load_file(Path::new("/tmp/does_not_exist_lab_editor_test.txt"));
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn load_new_format_single_step() {
        let f = tmp_file("1\necho hello\nhello\n");
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0], vec!["echo hello", "hello"]);
    }

    #[test]
    fn load_new_format_multiple_steps() {
        let f = tmp_file("1\nfirst line\n2\nsecond line\nmore output\n");
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], vec!["first line"]);
        assert_eq!(steps[1], vec!["second line", "more output"]);
    }

    #[test]
    fn load_new_format_with_number_in_output() {
        // First "2" is the step delimiter, second "2" is output in step 2
        let f = tmp_file("1\necho 2\n2\n2\nls\nfile.txt\n");
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], vec!["echo 2"]);
        assert_eq!(steps[1], vec!["2", "ls", "file.txt"]);
    }

    #[test]
    fn load_old_format_with_dollar_prefix() {
        let f = tmp_file("1\n$ echo hello\nhello\n$\n$ ls\nfile.txt\n");
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 1);
        // Flattened: all lines in one Vec<String>
        assert_eq!(steps[0], vec!["echo hello", "hello", "ls", "file.txt"]);
    }

    #[test]
    fn load_old_format_multi_step() {
        let content = "1\n$ cmd1\nout1\n$\n\n2\n$ cmd2\nout2\n";
        let f = tmp_file(content);
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], vec!["cmd1", "out1"]);
        assert_eq!(steps[1], vec!["cmd2", "out2"]);
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let steps = vec![
            vec!["echo hello".to_string(), "hello".to_string()],
            vec!["ls".to_string(), "file.txt".to_string()],
        ];
        let f = tempfile::NamedTempFile::new().unwrap();
        save_file(f.path(), &steps).unwrap();

        let loaded = load_file(f.path());
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0], vec!["echo hello", "hello"]);
        assert_eq!(loaded[1], vec!["ls", "file.txt"]);
    }

    #[test]
    fn save_format_correct_no_blank_lines() {
        let steps = vec![
            vec!["echo hi".to_string(), "hi".to_string()],
            vec!["pwd".to_string(), "/home".to_string()],
        ];
        let f = tempfile::NamedTempFile::new().unwrap();
        save_file(f.path(), &steps).unwrap();

        let content = fs::read_to_string(f.path()).unwrap();
        // NO blank lines between steps!
        assert_eq!(content, "1\necho hi\nhi\n2\npwd\n/home\n");
    }

    #[test]
    fn save_empty_step() {
        // Empty step: just the step number
        let steps = vec![vec![], vec!["cmd".to_string()]];
        let f = tempfile::NamedTempFile::new().unwrap();
        save_file(f.path(), &steps).unwrap();

        let content = fs::read_to_string(f.path()).unwrap();
        assert_eq!(content, "1\n2\ncmd\n");
    }

    #[test]
    fn save_empty_steps() {
        let steps: Vec<Step> = vec![];
        let f = tempfile::NamedTempFile::new().unwrap();
        save_file(f.path(), &steps).unwrap();

        let content = fs::read_to_string(f.path()).unwrap();
        assert_eq!(content, "\n");
    }

    #[test]
    fn load_legacy_blank_line_format() {
        // Legacy format with blank lines between steps
        let f = tmp_file("1\nfirst line\n\n2\nsecond line\n");
        let steps = load_file(f.path());
        // Should handle gracefully - blank lines are skipped
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], vec!["first line"]);
        assert_eq!(steps[1], vec!["second line"]);
    }

    #[test]
    fn load_old_format_with_man_page_numbers() {
        // Man page output contains bare digits like "1     General Commands Manual"
        // These must NOT be treated as step boundaries
        let content = "\
1\n\
$ man man\n\
MAN(1)     General Commands Manual     MAN(1)\n\
     1     General Commands Manual\n\
     2     System Calls Manual\n\
     3     Library Functions Manual\n\
$\n\
\n\
2\n\
$ man cat\n\
CAT(1)     General Commands Manual     CAT(1)\n\
$\n";
        let f = tmp_file(content);
        let steps = load_file(f.path());
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0][0], "man man");
        assert_eq!(steps[1][0], "man cat");
    }
}
