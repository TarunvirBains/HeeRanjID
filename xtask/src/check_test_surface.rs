use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
};

// ---------------------------------------------------------------------------
// Roots scanned for each surface type
// ---------------------------------------------------------------------------

/// Rust source surfaces that must remain active wherever they appear.
const ACTIVE_RUST_COMPONENTS: &[&str] = &["src", "tests", "examples", "benches"];

/// Base directory for Python tests; scanner looks for tests/ subdirs within.
const PYTHON_ROOT: &str = "bindings/python";

/// Vitest/Jest-style TypeScript test roots.
const TYPESCRIPT_TEST_ROOTS: &[&str] = &[
    "bindings/typescript/tests",
    "bindings/typescript/prisma/tests",
];

/// xUnit C# test roots.
const DOTNET_TEST_ROOTS: &[&str] = &["bindings/dotnet/tests"];

/// GitHub Actions workflow root.
const WORKFLOW_ROOTS: &[&str] = &[".github/workflows"];

// ---------------------------------------------------------------------------
// Banned patterns
// ---------------------------------------------------------------------------

/// Workflow command-line patterns that hide quarantined/ignored test lanes.
const WORKFLOW_QUARANTINE_PATTERNS: &[&str] = &[
    "--ignored",
    "--include-ignored",
    "run-ignored",
    "quarantine",
];

/// Known env-var names that gate legitimate integration-test skips.
const ENV_GATE_VARS: &[&str] = &["DATABASE_URL", "MSSQL_URL"];

#[derive(Clone, Copy)]
struct TypeScriptRunner {
    name: &'static str,
    skip_pattern: &'static str,
    only_pattern: &'static str,
}

const TYPESCRIPT_RUNNERS: &[TypeScriptRunner] = &[
    TypeScriptRunner {
        name: "describe",
        skip_pattern: "describe.skip",
        only_pattern: "describe.only",
    },
    TypeScriptRunner {
        name: "it",
        skip_pattern: "it.skip",
        only_pattern: "it.only",
    },
    TypeScriptRunner {
        name: "test",
        skip_pattern: "test.skip",
        only_pattern: "test.only",
    },
    TypeScriptRunner {
        name: "suite",
        skip_pattern: "suite.skip",
        only_pattern: "suite.only",
    },
];

// ---------------------------------------------------------------------------
// Finding type
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Finding {
    path: PathBuf,
    line: usize,
    pattern: &'static str,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(list_only: bool) -> ExitCode {
    // --- Collect files ---
    let mut rust_files = Vec::new();
    if let Err(error) = collect_active_rs_files(Path::new("."), Path::new("."), &mut rust_files) {
        eprintln!(".: failed to walk Rust active surfaces: {error}");
        return ExitCode::FAILURE;
    }
    rust_files.sort();
    rust_files.dedup();

    let mut python_files = Vec::new();
    let python_root = Path::new(PYTHON_ROOT);
    if python_root.exists()
        && let Err(error) = collect_python_test_files(python_root, &mut python_files)
    {
        eprintln!("{}: failed to walk: {error}", display_path(python_root));
        return ExitCode::FAILURE;
    }
    python_files.sort();
    python_files.dedup();

    let mut typescript_files = Vec::new();
    for root in TYPESCRIPT_TEST_ROOTS.iter().map(Path::new) {
        if root.exists()
            && let Err(error) =
                collect_files_with_extensions(root, &["ts", "js"], &mut typescript_files)
        {
            eprintln!("{}: failed to walk: {error}", display_path(root));
            return ExitCode::FAILURE;
        }
    }
    typescript_files.sort();
    typescript_files.dedup();

    let mut dotnet_files = Vec::new();
    for root in DOTNET_TEST_ROOTS.iter().map(Path::new) {
        if root.exists()
            && let Err(error) = collect_files_with_extensions(root, &["cs"], &mut dotnet_files)
        {
            eprintln!("{}: failed to walk: {error}", display_path(root));
            return ExitCode::FAILURE;
        }
    }
    dotnet_files.sort();
    dotnet_files.dedup();

    let mut workflow_files = Vec::new();
    for root in WORKFLOW_ROOTS.iter().map(Path::new) {
        if root.exists()
            && let Err(error) = collect_workflow_files(root, &mut workflow_files)
        {
            eprintln!("{}: failed to walk: {error}", display_path(root));
            return ExitCode::FAILURE;
        }
    }
    workflow_files.sort();

    // --- Scan files ---
    let mut findings = Vec::new();

    for path in &rust_files {
        match fs::read_to_string(path) {
            Ok(source) => findings.extend(scan_rust_no_quarantine(path, &source)),
            Err(error) => {
                eprintln!("{}: failed to read: {error}", display_path(path));
                return ExitCode::FAILURE;
            }
        }
    }

    for path in &python_files {
        match fs::read_to_string(path) {
            Ok(source) => findings.extend(scan_python_no_quarantine(path, &source)),
            Err(error) => {
                eprintln!("{}: failed to read: {error}", display_path(path));
                return ExitCode::FAILURE;
            }
        }
    }

    for path in &typescript_files {
        match fs::read_to_string(path) {
            Ok(source) => findings.extend(scan_typescript_no_quarantine(path, &source)),
            Err(error) => {
                eprintln!("{}: failed to read: {error}", display_path(path));
                return ExitCode::FAILURE;
            }
        }
    }

    for path in &dotnet_files {
        match fs::read_to_string(path) {
            Ok(source) => findings.extend(scan_dotnet_no_quarantine(path, &source)),
            Err(error) => {
                eprintln!("{}: failed to read: {error}", display_path(path));
                return ExitCode::FAILURE;
            }
        }
    }

    for path in &workflow_files {
        match fs::read_to_string(path) {
            Ok(source) => findings.extend(scan_workflow_no_quarantine(path, &source)),
            Err(error) => {
                eprintln!("{}: failed to read: {error}", display_path(path));
                return ExitCode::FAILURE;
            }
        }
    }

    // --- Report ---
    let total_files = rust_files.len()
        + python_files.len()
        + typescript_files.len()
        + dotnet_files.len()
        + workflow_files.len();

    if list_only {
        let paths: BTreeSet<_> = findings.iter().map(|f| f.path.clone()).collect();
        for path in &paths {
            println!("{}", display_path(path));
        }
    } else {
        for finding in &findings {
            eprintln!(
                "{}:{}: forbidden test-surface pattern `{}`",
                display_path(&finding.path),
                finding.line,
                finding.pattern,
            );
        }
        eprintln!(
            "check-test-surface: scanned {} files ({} Rust, {} Python, {} TypeScript, {} .NET, {} workflow); {} violations",
            total_files,
            rust_files.len(),
            python_files.len(),
            typescript_files.len(),
            dotnet_files.len(),
            workflow_files.len(),
            findings.len(),
        );
    }

    if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ---------------------------------------------------------------------------
// File collectors
// ---------------------------------------------------------------------------

fn collect_active_rs_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let canonical_root = fs::canonicalize(root)?;
    let canonical_dir = fs::canonicalize(dir)?;

    if !canonical_dir.starts_with(&canonical_root) {
        return Ok(());
    }

    for entry in fs::read_dir(&canonical_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "target" || name.as_encoded_bytes().starts_with(b"."))
            {
                continue;
            }

            let canonical_child = fs::canonicalize(&path)?;
            if !canonical_child.starts_with(&canonical_root) {
                continue;
            }

            collect_active_rs_files(&canonical_root, &canonical_child, files)?;
        } else if file_type.is_file()
            && path.extension().is_some_and(|ext| ext == "rs")
            && is_active_rust_surface(&canonical_root, &path)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn is_active_rust_surface(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    relative.components().any(|component| {
        let name = component.as_os_str();
        ACTIVE_RUST_COMPONENTS
            .iter()
            .any(|active_component| name == *active_component)
    })
}

fn collect_workflow_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            collect_workflow_files(&path, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "yml" || ext == "yaml")
        {
            files.push(path);
        }
    }
    Ok(())
}

/// Collect `.py` files from any `tests/` directory anywhere under `root`.
///
/// Matches the glob `bindings/python/**/tests/**/*.py`: recurses into
/// non-`tests` directories looking for `tests/` subdirs, then collects all
/// `.py` files recursively inside each `tests/` dir.
fn collect_python_test_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                // Inside a tests/ dir: collect all .py files recursively.
                collect_py_files_recursive(&path, files)?;
            } else if path.file_name().is_none_or(|name| name != "__pycache__") {
                // Outside a tests/ dir: recurse looking for more tests/ subdirs.
                collect_python_test_files(&path, files)?;
            }
        }
    }
    Ok(())
}

fn collect_py_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if path.file_name().is_none_or(|name| name != "__pycache__") {
                collect_py_files_recursive(&path, files)?;
            }
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "py") {
            files.push(path);
        }
    }
    Ok(())
}

fn collect_files_with_extensions(
    root: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let skip = path
                .file_name()
                .is_some_and(|name| name == "node_modules" || name == "bin" || name == "obj");
            if !skip {
                collect_files_with_extensions(&path, extensions, files)?;
            }
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
        {
            files.push(path);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rust scanner: ban #[ignore], allow #[should_panic]
// ---------------------------------------------------------------------------

fn scan_rust_no_quarantine(path: &Path, source: &str) -> Vec<Finding> {
    let stripped = strip_comments_and_literals(source);
    let bytes = stripped.as_bytes();
    let mut findings = Vec::new();
    let mut offset = 0;

    while let Some(relative_index) = stripped[offset..].find('#') {
        let hash_index = offset + relative_index;
        let mut cursor = hash_index + 1;

        if bytes.get(cursor) == Some(&b'!') {
            cursor += 1;
        }

        cursor = skip_ascii_whitespace(bytes, cursor);
        if bytes.get(cursor) != Some(&b'[') {
            offset = cursor;
            continue;
        }

        let Some(attr_end) = find_attribute_end(bytes, cursor) else {
            offset = cursor + 1;
            continue;
        };

        let attr = &stripped[cursor + 1..attr_end];
        if attr_starts_with_ident(attr, "ignore")
            || (attr_starts_with_ident(attr, "cfg_attr")
                && cfg_attr_payload_contains_ident(attr, "ignore"))
        {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_number_at(&stripped, hash_index),
                pattern: "#[ignore]",
            });
        }

        offset = attr_end + 1;
    }

    findings
}

/// Detect `#[ignore]` (including whitespace variants) but not `#[should_panic]`.
///
/// Matches `# [ ignore ]`, `# [ ignore = "..." ]`, and `# [ ignore ( ... ) ]`
/// but rejects `#[ignored_helper]` (ident continues after `ignore`).
#[cfg(test)]
fn contains_ignore_attr(line: &str) -> bool {
    !scan_rust_no_quarantine(Path::new("inline.rs"), line).is_empty()
}

// ---------------------------------------------------------------------------
// Workflow scanner: ban quarantine/ignored command-line patterns
// ---------------------------------------------------------------------------

fn scan_workflow_no_quarantine(path: &Path, source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }

        let active = strip_yaml_inline_comment(line);
        for pattern in WORKFLOW_QUARANTINE_PATTERNS {
            if workflow_line_has_pattern(active, pattern) {
                findings.push(Finding {
                    path: path.to_owned(),
                    line: line_index + 1,
                    pattern,
                });
            }
        }
    }

    findings
}

fn strip_yaml_inline_comment(line: &str) -> &str {
    line.find(" #")
        .map_or(line, |comment_start| &line[..comment_start])
}

fn workflow_line_has_pattern(line: &str, pattern: &str) -> bool {
    if pattern == "quarantine" {
        contains_identifier(&line.to_ascii_lowercase(), pattern)
    } else {
        line.contains(pattern)
    }
}

// ---------------------------------------------------------------------------
// Python scanner: ban unconditional skips, allow env-gated skips
// ---------------------------------------------------------------------------

/// Scan a Python test file for banned skip patterns.
///
/// **Banned:**
/// - `@pytest.mark.skip` (bare) and non-env-gated `skipif`.
/// - `@unittest.skip` / `unittest.skip(` and non-env-gated `skipIf` /
///   `skipUnless`.
/// - `pytest.skip(...)` that is NOT inside a recognised env-var gate block
///   (`if DATABASE_URL is None:`, `if MSSQL_URL is None:`, `if not DATABASE_URL`, ...)
///   with `allow_module_level=True`.
/// - Any line containing the word `quarantine` in active (non-comment) code.
///
/// **Allowed:**
/// - `@pytest.mark.skipif(...)` only when it skips on missing env.
/// - `pytest.importorskip(...)` - driver/browser import gate.
/// - `pytest.skip(..., allow_module_level=True)` inside a missing-env gate block
///   (the `allow_module_level` argument may appear on a continuation line).
fn scan_python_no_quarantine(path: &Path, source: &str) -> Vec<Finding> {
    let lines: Vec<&str> = source.lines().collect();
    let mut findings = Vec::new();
    let mut in_env_gate = false;
    let mut env_gate_indent: usize = 0;

    for (line_idx, &line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;

        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        // Blank lines do not change env-gate state.
        if trimmed.is_empty() {
            continue;
        }

        // Exit the env-gate block when we reach a non-blank line at or above
        // the gate opener's indentation level.
        if in_env_gate && indent <= env_gate_indent {
            in_env_gate = false;
        }

        // Check for a new env-gate opener.
        if !in_env_gate && is_env_gate_opener(trimmed) {
            in_env_gate = true;
            env_gate_indent = indent;
        }

        // Strip Python line comment for active-code checks.
        let active = strip_python_comment(line);

        // 1. @pytest.mark.skip (bare, not skipif).
        if is_bare_mark_skip(active) {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_number,
                pattern: "@pytest.mark.skip",
            });
        }

        // 2. pytest.mark.skipif is allowed only for missing-env gates.
        if active.contains("pytest.mark.skipif") {
            let call_text = collect_python_call_text(active, &lines, line_idx);
            let condition = first_python_call_arg(&call_text);
            if !is_missing_env_gate_condition(condition) {
                findings.push(Finding {
                    path: path.to_owned(),
                    line: line_number,
                    pattern: "@pytest.mark.skipif",
                });
            }
        }

        // 3. Unconditional unittest.skip.
        if contains_unconditional_unittest_skip(active) {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_number,
                pattern: "unittest.skip",
            });
        }

        // 4. unittest skipIf/skipUnless are allowed only for missing-env gates.
        if contains_conditional_unittest_skip(active) {
            let call_text = collect_python_call_text(active, &lines, line_idx);
            let condition = first_python_call_arg(&call_text);
            if !is_missing_env_gate_condition(condition) {
                findings.push(Finding {
                    path: path.to_owned(),
                    line: line_number,
                    pattern: "unittest.skipIf",
                });
            }
        }

        // 5. pytest.skip( - allowed only inside env gate with allow_module_level=True.
        //    The allow_module_level argument may appear on a continuation line, so
        //    collect the full call text (up to 10 look-ahead lines) before deciding.
        if active.contains("pytest.skip(") {
            let call_text = collect_python_call_text(active, &lines, line_idx);
            let has_allow = call_text.contains("allow_module_level=True");
            let gated = in_env_gate && has_allow;
            if !gated {
                findings.push(Finding {
                    path: path.to_owned(),
                    line: line_number,
                    pattern: "pytest.skip",
                });
            }
        }

        // 6. Quarantine-list naming in active code.
        if active.to_ascii_lowercase().contains("quarantine") {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_number,
                pattern: "quarantine",
            });
        }
    }

    findings
}

fn scan_typescript_no_quarantine(path: &Path, source: &str) -> Vec<Finding> {
    let stripped = strip_comments_and_literals(source);
    let mut findings = Vec::new();

    for (byte_index, pattern) in typescript_runner_skip_focus_patterns(&stripped) {
        findings.push(Finding {
            path: path.to_owned(),
            line: line_number_at(&stripped, byte_index),
            pattern,
        });
    }

    for (line_index, line) in stripped.lines().enumerate() {
        if line.to_ascii_lowercase().contains("quarantine") {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_index + 1,
                pattern: "quarantine",
            });
        }
    }

    findings
}

fn scan_dotnet_no_quarantine(path: &Path, source: &str) -> Vec<Finding> {
    let stripped = strip_comments_and_literals(source);
    let bytes = stripped.as_bytes();
    let mut findings = Vec::new();
    let mut offset = 0;

    while let Some(relative_index) = stripped[offset..].find('[') {
        let attr_start = offset + relative_index;
        let Some(attr_end) = find_attribute_end(bytes, attr_start) else {
            offset = attr_start + 1;
            continue;
        };

        let attr = &stripped[attr_start + 1..attr_end];
        let pattern = if attr_starts_with_ident(attr, "Ignore") {
            Some("[Ignore]")
        } else if attr_starts_with_ident(attr, "Explicit") {
            Some("[Explicit]")
        } else if (attr_starts_with_ident(attr, "Fact") || attr_starts_with_ident(attr, "Theory"))
            && contains_identifier(attr, "Skip")
        {
            Some("Skip")
        } else {
            None
        };

        if let Some(pattern) = pattern {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_number_at(&stripped, attr_start),
                pattern,
            });
        }

        offset = attr_end + 1;
    }

    for (line_index, line) in stripped.lines().enumerate() {
        if line.to_ascii_lowercase().contains("quarantine") {
            findings.push(Finding {
                path: path.to_owned(),
                line: line_index + 1,
                pattern: "quarantine",
            });
        }
    }

    findings
}

/// Collect the text of a Python call that may span multiple lines by tracking
/// unmatched open parentheses.  Starts from `first_line_active` (the active
/// portion of the line on which the call opens, already comment-stripped) and
/// appends up to 10 continuation lines from `all_lines[start_idx + 1..]`.
fn collect_python_call_text(
    first_line_active: &str,
    all_lines: &[&str],
    start_idx: usize,
) -> String {
    let mut text = first_line_active.to_owned();

    // Count net open parens on the first line.
    let mut depth: i32 = 0;
    for ch in first_line_active.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
    }

    if depth <= 0 {
        return text; // Call is already closed on this line.
    }

    // Look ahead until balanced or 10 extra lines consumed.
    let limit = (start_idx + 1 + 10).min(all_lines.len());
    for continuation in &all_lines[start_idx + 1..limit] {
        let active = strip_python_comment(continuation);
        text.push('\n');
        text.push_str(active);
        for ch in active.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        if depth <= 0 {
            break;
        }
    }

    text
}

fn first_python_call_arg(call_text: &str) -> &str {
    let Some(open_paren) = call_text.find('(') else {
        return call_text;
    };

    let bytes = call_text.as_bytes();
    let mut depth = 0usize;
    let mut cursor = open_paren + 1;
    let arg_start = cursor;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' if depth == 0 => return call_text[arg_start..cursor].trim(),
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => return call_text[arg_start..cursor].trim(),
            _ => {}
        }
        cursor += 1;
    }

    call_text[arg_start..].trim()
}

/// Returns `true` if `trimmed` is an `if` statement that references a known
/// missing-env gate variable (`DATABASE_URL`, `MSSQL_URL`).
fn is_env_gate_opener(trimmed: &str) -> bool {
    if !trimmed.starts_with("if ") && !trimmed.starts_with("if\t") {
        return false;
    }
    is_missing_env_gate_condition(trimmed)
}

fn is_missing_env_gate_condition(text: &str) -> bool {
    let normalized = normalize_python_gate_text(text);
    !contains_present_env_gate_condition(&normalized)
        && ENV_GATE_VARS
            .iter()
            .any(|var| contains_missing_env_gate_for_var(&normalized, var))
}

fn contains_present_env_gate_condition(normalized: &str) -> bool {
    ENV_GATE_VARS.iter().any(|var| {
        let present_patterns = [
            format!("{var} is not None"),
            format!("{var} != None"),
            format!("{var} != \"\""),
            format!("{var} != ''"),
            format!("not {var} is None"),
            format!("not ({var} is None"),
        ];
        present_patterns
            .iter()
            .any(|pattern| normalized.contains(pattern))
    })
}

fn contains_missing_env_gate_for_var(normalized: &str, var: &str) -> bool {
    let missing_patterns = [
        format!("{var} is None"),
        format!("{var} == None"),
        format!("{var} == \"\""),
        format!("{var} == ''"),
        format!("not {var}"),
    ];
    missing_patterns
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

fn normalize_python_gate_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip a Python line comment (simple heuristic: everything after `#`).
///
/// May misfire if `#` appears inside a string literal, but that is not
/// possible for the banned patterns we check (they contain no `#`).
fn strip_python_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before)
}

/// Returns `true` if `active` contains `pytest.mark.skip` NOT followed by `if`
/// (i.e., bare `@pytest.mark.skip`, not `@pytest.mark.skipif`).
fn is_bare_mark_skip(active: &str) -> bool {
    let marker = "pytest.mark.skip";
    let mut offset = 0;
    while let Some(rel_idx) = active[offset..].find(marker) {
        let idx = offset + rel_idx;
        let after = &active[idx + marker.len()..];
        if !after.starts_with("if") {
            return true;
        }
        offset = idx + marker.len();
    }
    false
}

/// Returns `true` if `active` contains `unittest.skip(` but NOT `unittest.skipIf(`
/// or `unittest.skipUnless(` (those are conditional and remain allowed).
fn contains_unconditional_unittest_skip(active: &str) -> bool {
    let marker = "unittest.skip";
    let mut offset = 0;
    while let Some(rel_idx) = active[offset..].find(marker) {
        let idx = offset + rel_idx;
        let after = &active[idx + marker.len()..];
        if !after.starts_with("If") && !after.starts_with("Unless") {
            return true;
        }
        offset = idx + marker.len();
    }
    false
}

fn typescript_runner_skip_focus_patterns(source: &str) -> Vec<(usize, &'static str)> {
    let mut patterns = Vec::new();

    for runner in TYPESCRIPT_RUNNERS {
        let mut offset = 0;
        while let Some(relative_index) = source[offset..].find(runner.name) {
            let start = offset + relative_index;
            let after = start + runner.name.len();

            if is_typescript_ident_boundary(source, start, after) {
                collect_typescript_chain_patterns(source, after, *runner, &mut patterns);
            }

            offset = after;
        }
    }

    patterns
}

fn collect_typescript_chain_patterns(
    source: &str,
    mut cursor: usize,
    runner: TypeScriptRunner,
    patterns: &mut Vec<(usize, &'static str)>,
) {
    let bytes = source.as_bytes();

    loop {
        cursor = skip_ascii_whitespace(bytes, cursor);

        match bytes.get(cursor) {
            Some(b'.') => {
                cursor += 1;
                cursor = skip_ascii_whitespace(bytes, cursor);
                let Some((segment, next_cursor)) = parse_typescript_ident(source, cursor) else {
                    return;
                };

                if segment == "skip" || segment == "skipIf" {
                    patterns.push((cursor, runner.skip_pattern));
                } else if segment == "only" {
                    patterns.push((cursor, runner.only_pattern));
                }
                cursor = next_cursor;
            }
            Some(b'(') => {
                let Some(next_cursor) = skip_balanced_parentheses(bytes, cursor) else {
                    return;
                };
                cursor = next_cursor;
            }
            _ => return,
        }
    }
}

fn is_typescript_ident_boundary(line: &str, start: usize, after: usize) -> bool {
    let bytes = line.as_bytes();
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(after);
    !before.is_some_and(|byte| is_ident_byte(*byte) || *byte == b'$')
        && !after.is_some_and(|byte| is_ident_byte(*byte) || *byte == b'$')
}

fn parse_typescript_ident(line: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = line.as_bytes();
    let first = *bytes.get(start)?;
    if !is_ident_byte(first) && first != b'$' {
        return None;
    }

    let mut cursor = start + 1;
    while bytes
        .get(cursor)
        .is_some_and(|byte| is_ident_byte(*byte) || *byte == b'$')
    {
        cursor += 1;
    }

    Some((&line[start..cursor], cursor))
}

fn skip_balanced_parentheses(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes.get(start), Some(&b'('));
    let mut depth = 0usize;
    let mut cursor = start;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }

    None
}

// ---------------------------------------------------------------------------
// Rust comment / literal stripping (preserves newlines for line alignment)
// ---------------------------------------------------------------------------

/// Replace comments and string/char/raw-string literals with spaces so that
/// rustdoc `ignore` fences and prose do not become false positives, while
/// preserving the structure needed for line-number reporting.
pub(crate) fn strip_comments_and_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if starts_with(bytes, index, b"//") {
            index = mask_line_comment(bytes, index, &mut output);
        } else if starts_with(bytes, index, b"/*") {
            index = mask_block_comment(bytes, index, &mut output);
        } else if let Some(raw_start) = raw_string_start(bytes, index) {
            index = mask_raw_string(bytes, index, raw_start.hashes, &mut output);
        } else if starts_with(bytes, index, b"b\"") || starts_with(bytes, index, b"c\"") {
            index = mask_quoted(bytes, index, index + 1, b'"', &mut output);
        } else if bytes[index] == b'"' {
            index = mask_quoted(bytes, index, index, b'"', &mut output);
        } else if starts_with(bytes, index, b"b'") {
            index = mask_char_or_keep(bytes, index, index + 1, &mut output);
        } else if bytes[index] == b'\'' {
            index = mask_char_or_keep(bytes, index, index, &mut output);
        } else if bytes[index] == b'`' {
            index = mask_quoted(bytes, index, index, b'`', &mut output);
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(output).expect("masking preserves UTF-8")
}

#[derive(Clone, Copy)]
struct RawStringStart {
    hashes: usize,
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<RawStringStart> {
    let raw_index = if bytes.get(index) == Some(&b'r') {
        index
    } else if matches!(bytes.get(index), Some(b'b' | b'c')) && bytes.get(index + 1) == Some(&b'r') {
        index + 1
    } else {
        return None;
    };

    let mut cursor = raw_index + 1;
    let mut hashes = 0;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&b'"')).then_some(RawStringStart { hashes })
}

fn mask_raw_string(bytes: &[u8], start: usize, hashes: usize, output: &mut Vec<u8>) -> usize {
    let raw_index = if bytes[start] == b'r' {
        start
    } else {
        start + 1
    };
    let content_start = raw_index + 1 + hashes + 1;
    let mut cursor = content_start;

    while cursor < bytes.len() {
        if bytes[cursor] == b'"' && raw_hashes_match(bytes, cursor + 1, hashes) {
            return mask_range(bytes, start, cursor + 1 + hashes, output);
        }
        cursor += 1;
    }

    mask_range(bytes, start, bytes.len(), output)
}

fn raw_hashes_match(bytes: &[u8], start: usize, hashes: usize) -> bool {
    (0..hashes).all(|offset| bytes.get(start + offset) == Some(&b'#'))
}

fn mask_line_comment(bytes: &[u8], start: usize, output: &mut Vec<u8>) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor] != b'\n' {
        output.push(b' ');
        cursor += 1;
    }
    cursor
}

fn mask_block_comment(bytes: &[u8], start: usize, output: &mut Vec<u8>) -> usize {
    let mut cursor = start;
    let mut depth = 0usize;

    while cursor < bytes.len() {
        if starts_with(bytes, cursor, b"/*") {
            depth += 1;
            output.extend_from_slice(b"  ");
            cursor += 2;
        } else if starts_with(bytes, cursor, b"*/") {
            depth = depth.saturating_sub(1);
            output.extend_from_slice(b"  ");
            cursor += 2;
            if depth == 0 {
                break;
            }
        } else {
            mask_byte(bytes[cursor], output);
            cursor += 1;
        }
    }

    cursor
}

fn mask_quoted(
    bytes: &[u8],
    start: usize,
    quote_index: usize,
    quote: u8,
    output: &mut Vec<u8>,
) -> usize {
    let mut cursor = quote_index + 1;
    let mut escaped = false;

    while cursor < bytes.len() {
        if !escaped && bytes[cursor] == quote {
            return mask_range(bytes, start, cursor + 1, output);
        }
        escaped = !escaped && bytes[cursor] == b'\\';
        if bytes[cursor] != b'\\' {
            escaped = false;
        }
        cursor += 1;
    }

    mask_range(bytes, start, bytes.len(), output)
}

fn mask_char_or_keep(
    bytes: &[u8],
    start: usize,
    quote_index: usize,
    output: &mut Vec<u8>,
) -> usize {
    let mut cursor = quote_index + 1;
    let mut escaped = false;

    while cursor < bytes.len() && bytes[cursor] != b'\n' {
        if !escaped && bytes[cursor] == b'\'' {
            return mask_range(bytes, start, cursor + 1, output);
        }
        escaped = !escaped && bytes[cursor] == b'\\';
        if bytes[cursor] != b'\\' {
            escaped = false;
        }
        cursor += 1;
    }

    output.push(bytes[start]);
    start + 1
}

fn mask_range(bytes: &[u8], start: usize, end: usize, output: &mut Vec<u8>) -> usize {
    for byte in &bytes[start..end] {
        mask_byte(*byte, output);
    }
    end
}

fn mask_byte(byte: u8, output: &mut Vec<u8>) {
    if byte == b'\n' {
        output.push(b'\n');
    } else {
        output.push(b' ');
    }
}

// ---------------------------------------------------------------------------
// Byte-level helpers
// ---------------------------------------------------------------------------

fn find_attribute_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes.get(start), Some(&b'['));

    let mut depth = 0usize;
    let mut cursor = start;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'[' => depth += 1,
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }

    None
}

fn attr_starts_with_ident(attr: &str, ident: &str) -> bool {
    let bytes = attr.as_bytes();
    let start = skip_ascii_whitespace(bytes, 0);

    starts_with(bytes, start, ident.as_bytes())
        && !bytes
            .get(start + ident.len())
            .is_some_and(|byte| is_ident_byte(*byte))
}

fn cfg_attr_payload_contains_ident(attr: &str, ident: &str) -> bool {
    let Some(open_paren) = attr.find('(') else {
        return false;
    };
    let mut depth = 0usize;

    for (relative_index, byte) in attr.as_bytes()[open_paren + 1..].iter().enumerate() {
        match *byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                let payload_start = open_paren + 1 + relative_index + 1;
                return contains_identifier(&attr[payload_start..], ident);
            }
            _ => {}
        }
    }

    false
}

fn contains_conditional_unittest_skip(active: &str) -> bool {
    active.contains("unittest.skipIf") || active.contains("unittest.skipUnless")
}

fn contains_identifier(source: &str, ident: &str) -> bool {
    let mut offset = 0;

    while let Some(relative_index) = source[offset..].find(ident) {
        let index = offset + relative_index;
        let before = index.checked_sub(1).and_then(|i| source.as_bytes().get(i));
        let after_index = index + ident.len();
        let after = source.as_bytes().get(after_index);

        if !before.is_some_and(|byte| is_ident_byte(*byte))
            && !after.is_some_and(|byte| is_ident_byte(*byte))
        {
            return true;
        }

        offset = after_index;
    }

    false
}

fn line_number_at(source: &str, index: usize) -> usize {
    source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn skip_ascii_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|b| b.is_ascii_whitespace()) {
        cursor += 1;
    }
    cursor
}

fn is_ident_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes.get(index..index + needle.len()) == Some(needle)
}

fn display_path(path: &Path) -> String {
    let current_dir = std::env::current_dir().ok();
    let display = current_dir
        .as_deref()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path);
    display.display().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        contains_ignore_attr, scan_dotnet_no_quarantine, scan_python_no_quarantine,
        scan_rust_no_quarantine, scan_typescript_no_quarantine, scan_workflow_no_quarantine,
        strip_comments_and_literals,
    };
    use std::path::Path;

    // -- Rust: #[ignore] detection ------------------------------------------

    #[test]
    fn rust_rejects_ignore_attribute() {
        let source = r#"
#[test]
#[ignore]
async fn hidden() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].pattern, "#[ignore]");
    }

    #[test]
    fn rust_rejects_ignore_attribute_with_whitespace() {
        let source = r#"
#[ ignore = "needs database" ]
async fn hidden() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
    }

    #[test]
    fn rust_rejects_multiline_ignore_attribute() {
        let source = r#"
#[test]
#[
    ignore
]
async fn hidden() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].pattern, "#[ignore]");
    }

    #[test]
    fn rust_rejects_cfg_attr_ignore_attribute() {
        let source = r#"
#[test]
#[cfg_attr(feature = "slow-tests", ignore)]
fn hidden() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].pattern, "#[ignore]");
    }

    #[test]
    fn rust_allows_should_panic_expected() {
        let source = r#"
#[test]
#[should_panic(expected = "out of range")]
fn panics_on_overflow() { heeranjid::HeerId::from_u128(u128::MAX); }
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty(), "should_panic must not be flagged");
    }

    #[test]
    fn rust_allows_identifier_containing_ignore() {
        // `ignored_prefix` must not be flagged.
        let source = r#"
fn ignored_prefix_test() {}
let _ = IGNORE_ME;
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty());
    }

    // -- Rust: comment / literal stripping ---------------------------------

    #[test]
    fn rust_ignores_ignore_in_line_comment() {
        let source = r#"
// #[ignore]
#[test]
fn visible() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty(), "comment must be stripped");
    }

    #[test]
    fn rust_ignores_ignore_in_string_literal() {
        let source = r##"
let s = "#[ignore]";
#[test]
fn visible() {}
"##;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty(), "string literal must be stripped");
    }

    #[test]
    fn rust_ignores_ignore_in_rustdoc_fence() {
        // Rustdoc ```ignore``` fences appear as `//!` or `///` comments.
        let source = r#"
//! ```ignore
//! let x = HeerId::new();
//! ```
#[test]
fn visible() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty(), "rustdoc fence must be stripped");
    }

    #[test]
    fn rust_ignores_ignore_in_raw_string() {
        let source = r##"
let s = r#"#[ignore]"#;
"##;
        let stripped = strip_comments_and_literals(source);
        assert!(!stripped.contains("ignore"));
    }

    #[test]
    fn rust_ignores_ignore_in_block_comment() {
        let source = r#"
/* #[ignore] */
#[test]
fn visible() {}
"#;
        let findings = scan_rust_no_quarantine(Path::new("heeranjid/src/lib.rs"), source);
        assert!(findings.is_empty(), "block comment must be stripped");
    }

    // -- Workflow: quarantine / ignored patterns ----------------------------

    #[test]
    fn workflow_rejects_ignored_flag() {
        let source = r#"
name: CI
jobs:
  test:
    steps:
      - run: cargo test -- --ignored
"#;
        let findings = scan_workflow_no_quarantine(Path::new(".github/workflows/ci.yml"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].pattern, "--ignored");
    }

    #[test]
    fn workflow_rejects_include_ignored_flag() {
        let source = r#"
      - run: cargo test -- --include-ignored
"#;
        let findings = scan_workflow_no_quarantine(Path::new(".github/workflows/ci.yml"), source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].pattern, "--include-ignored");
    }

    #[test]
    fn workflow_rejects_run_ignored_and_quarantine_word() {
        let source = r#"
      - run: cargo xtask run-ignored
      - run: cargo test --features quarantine-list
      - run: cargo test --features Quarantine-list
"#;
        let findings = scan_workflow_no_quarantine(Path::new(".github/workflows/ci.yml"), source);
        let patterns: Vec<_> = findings.iter().map(|finding| finding.pattern).collect();
        assert!(patterns.contains(&"run-ignored"));
        assert_eq!(
            patterns
                .iter()
                .filter(|pattern| **pattern == "quarantine")
                .count(),
            2
        );
    }

    #[test]
    fn workflow_allows_ignored_in_comment() {
        // `#` in YAML starts a comment; the pattern after it must be ignored.
        let source = r#"
      - run: cargo test # --include-ignored in comment
"#;
        let findings = scan_workflow_no_quarantine(Path::new(".github/workflows/ci.yml"), source);
        assert!(findings.is_empty(), "YAML comment must not be flagged");
    }

    #[test]
    fn workflow_allows_quarantined_prose_and_yaml_hash_literals() {
        let source = r#"
# --ignored in pure comment
      - name: quarantined prose in a step label
      - run: printf '%s\n' "tag#not-a-comment"
"#;
        let findings = scan_workflow_no_quarantine(Path::new(".github/workflows/ci.yml"), source);
        assert!(
            findings.is_empty(),
            "comments, non-matching prose, and quoted hash data must not be flagged: {findings:?}"
        );
    }

    // -- TypeScript and .NET active tests ---------------------------------

    #[test]
    fn typescript_rejects_vitest_skip_forms() {
        let source = r#"
import { describe, it } from "vitest";

describe.skip("suite", () => {
  it("case", () => {});
});

it.skip("single case", () => {});
"#;
        let findings =
            scan_typescript_no_quarantine(Path::new("bindings/typescript/tests/t.test.ts"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(patterns.contains(&"describe.skip"));
        assert!(patterns.contains(&"it.skip"));
    }

    #[test]
    fn typescript_rejects_vitest_only_forms() {
        let source = r#"
import { describe, it } from "vitest";

describe.only("suite", () => {
  it.only("case", () => {});
});
"#;
        let findings =
            scan_typescript_no_quarantine(Path::new("bindings/typescript/tests/t.test.ts"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(patterns.contains(&"describe.only"));
        assert!(patterns.contains(&"it.only"));
    }

    #[test]
    fn typescript_rejects_chained_vitest_focus_and_skip_forms() {
        let source = r#"
import { describe, it, test } from "vitest";

it.concurrent.only("case", () => {});
test.each([1, 2]).only("case %i", () => {});
describe.each([["pg"]]).only("suite %s", () => {});
test.skipIf(process.env.CI)("hidden", () => {});
"#;
        let findings =
            scan_typescript_no_quarantine(Path::new("bindings/typescript/tests/t.test.ts"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(patterns.contains(&"it.only"));
        assert!(patterns.contains(&"test.only"));
        assert!(patterns.contains(&"describe.only"));
        assert!(patterns.contains(&"test.skip"));
    }

    #[test]
    fn typescript_rejects_multiline_chained_vitest_focus_forms() {
        let source = r#"
import { describe, test } from "vitest";

test.each([
  [1],
  [2],
]).only("case %i", () => {});

describe
  .each([
    ["pg"],
  ])
  .only("suite %s", () => {});
"#;
        let findings =
            scan_typescript_no_quarantine(Path::new("bindings/typescript/tests/t.test.ts"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(patterns.contains(&"test.only"));
        assert!(patterns.contains(&"describe.only"));
    }

    #[test]
    fn typescript_ignores_skip_in_comments_and_literals() {
        let source = r#"
// it.skip("commented")
const text = `test.skip("not active")`;
it("runs", () => {});
"#;
        let findings =
            scan_typescript_no_quarantine(Path::new("bindings/typescript/tests/t.test.ts"), source);
        assert!(
            findings.is_empty(),
            "comments and literals must not be flagged"
        );
    }

    #[test]
    fn dotnet_rejects_xunit_skip_forms() {
        let source = r#"
using Xunit;

public class Tests
{
    [Fact(Skip = "not yet")]
    public void Hidden() {}

    [Theory(
        Skip = "not yet"
    )]
    public void AlsoHidden(int value) {}
}
"#;
        let findings =
            scan_dotnet_no_quarantine(Path::new("bindings/dotnet/tests/Tests.cs"), source);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].pattern, "Skip");
        assert_eq!(findings[1].pattern, "Skip");
    }

    // -- Python: allowed env-gated skips -----------------------------------

    #[test]
    fn python_allows_skipif_database_url() {
        let source = r#"
import os
import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
pytestmark = pytest.mark.skipif(
    DATABASE_URL is None,
    reason="DATABASE_URL not set",
)
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "skipif(DATABASE_URL is None) must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn python_allows_pytest_skip_module_level_inside_env_gate() {
        let source = r#"
import os
import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
if DATABASE_URL is None:
    pytest.skip("DATABASE_URL not set", allow_module_level=True)
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "env-gated pytest.skip with allow_module_level must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn python_allows_mssql_url_gate() {
        let source = r#"
import os
import pytest

MSSQL_URL = os.environ.get("MSSQL_URL")
if MSSQL_URL is None:
    pytest.skip("MSSQL_URL not set", allow_module_level=True)
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(findings.is_empty(), "MSSQL_URL gate must not be flagged");
    }

    #[test]
    fn python_allows_compound_missing_env_gate() {
        let source = r#"
import os
import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
MSSQL_URL = os.environ.get("MSSQL_URL")
if not DATABASE_URL or not MSSQL_URL:
    pytest.skip(
        "both database URLs required",
        allow_module_level=True,
    )
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "compound missing-env gate must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn python_allows_importorskip() {
        let source = r#"
import pytest
psycopg2 = pytest.importorskip("psycopg2")
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "importorskip must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn python_allows_decorator_skipif_env_var() {
        let source = r#"
import os, pytest
DATABASE_URL = os.environ.get("DATABASE_URL")
@pytest.mark.skipif(DATABASE_URL is None, reason="no db")
def test_something():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "skipif with env condition must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn python_rejects_skipif_without_known_env_gate() {
        let source = r#"
import pytest

@pytest.mark.skipif(True, reason="not wired")
def test_something():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"@pytest.mark.skipif"),
            "skipif without a CI-backed env gate must be flagged"
        );
    }

    #[test]
    fn python_rejects_skipif_when_env_is_present() {
        let source = r#"
import os, pytest
DATABASE_URL = os.environ.get("DATABASE_URL")
@pytest.mark.skipif(DATABASE_URL is not None, reason="wrong polarity")
def test_something():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"@pytest.mark.skipif"),
            "skipif(DATABASE_URL is not None) must be flagged"
        );
    }

    #[test]
    fn python_rejects_skipif_when_env_gate_only_appears_in_reason() {
        let source = r#"
import pytest

@pytest.mark.skipif(True, reason="DATABASE_URL is None")
def test_something():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"@pytest.mark.skipif"),
            "skipif reason text must not be treated as an env gate"
        );
    }

    // -- Python: banned unconditional skips --------------------------------

    #[test]
    fn python_rejects_unconditional_pytest_skip() {
        let source = r#"
import pytest

def test_slow():
    pytest.skip("not done yet")
    assert False
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"pytest.skip"),
            "unconditional pytest.skip must be flagged"
        );
    }

    #[test]
    fn python_rejects_bare_mark_skip_decorator() {
        let source = r#"
import pytest

@pytest.mark.skip
def test_broken():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"@pytest.mark.skip"),
            "bare @pytest.mark.skip must be flagged"
        );
    }

    #[test]
    fn python_rejects_bare_mark_skip_with_reason() {
        // @pytest.mark.skip(reason="...") - still unconditional.
        let source = r#"
@pytest.mark.skip(reason="needs refactor")
def test_wip():
    pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"@pytest.mark.skip"),
            "@pytest.mark.skip(reason=...) must be flagged"
        );
    }

    #[test]
    fn python_rejects_unittest_skip() {
        let source = r#"
import unittest

@unittest.skip("not yet")
class TestSuite(unittest.TestCase):
    def test_something(self):
        pass
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"unittest.skip"),
            "@unittest.skip must be flagged"
        );
    }

    #[test]
    fn python_rejects_module_level_skip_without_gate() {
        // pytest.skip with allow_module_level=True but no env-var gate.
        let source = r#"
import pytest
pytest.skip("temporarily disabled", allow_module_level=True)
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"pytest.skip"),
            "module-level skip without gate must be flagged"
        );
    }

    #[test]
    fn python_rejects_module_level_skip_inside_present_env_gate() {
        let source = r#"
import os
import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
if DATABASE_URL is not None:
    pytest.skip("wrong polarity", allow_module_level=True)
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"pytest.skip"),
            "present-env pytest.skip gate must be flagged"
        );
    }

    #[test]
    fn python_rejects_quarantine_naming() {
        let source = r#"
QUARANTINE_IDS = ["test_slow", "test_flaky"]
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        let patterns: Vec<_> = findings.iter().map(|f| f.pattern).collect();
        assert!(
            patterns.contains(&"quarantine"),
            "quarantine naming must be flagged"
        );
    }

    #[test]
    fn python_does_not_flag_quarantine_in_comment() {
        // If "quarantine" appears only in a Python comment, it must NOT be flagged.
        let source = r#"
# This test is not in the quarantine list
def test_clean():
    assert True
"#;
        let findings =
            scan_python_no_quarantine(Path::new("bindings/python/django/tests/t.py"), source);
        assert!(
            findings.is_empty(),
            "quarantine in Python comment must not be flagged: {findings:?}"
        );
    }

    // -- contains_ignore_attr unit tests -----------------------------------

    #[test]
    fn ignore_attr_basic() {
        assert!(contains_ignore_attr("#[ignore]"));
        assert!(contains_ignore_attr("#[ ignore ]"));
        assert!(contains_ignore_attr("#[ignore = \"reason\"]"));
    }

    #[test]
    fn ignore_attr_rejects_longer_ident() {
        assert!(!contains_ignore_attr("fn ignored_helper() {}"));
        assert!(!contains_ignore_attr("#[ignored]"));
    }

    #[test]
    fn ignore_attr_does_not_flag_should_panic() {
        assert!(!contains_ignore_attr(
            "#[should_panic(expected = \"oops\")]"
        ));
    }
}
