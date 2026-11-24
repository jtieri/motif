//! Runs `motif import` as a user would.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn themes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../motif-core/tests/fixtures/omarchy/themes")
}

fn theme(name: &str) -> PathBuf {
    themes().join(name)
}

/// Runs motif with a clean environment whose configuration folder is
/// `config`, so nothing reaches the real one.
fn motif(config: &Path, args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_motif"))
        .args(args)
        .env_clear()
        .env("HOME", config)
        .env("XDG_CONFIG_HOME", config)
        .output()
        .expect("motif runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 stderr")
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("folder")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn imports_every_built_in_theme() {
    let tmp = TempDir::new().expect("temp dir");
    let out = tmp.path();
    let mut count = 0;
    let mut dirs: Vec<PathBuf> = fs::read_dir(themes())
        .expect("themes")
        .map(|e| e.expect("entry").path())
        .collect();
    dirs.sort();
    for dir in &dirs {
        let name = dir
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let output = motif(out, &[&"import", &"--output-dir", &out, &dir]);
        assert!(output.status.success(), "{name}: {}", stderr(&output));
        let file = out.join(format!("{name}.json"));
        let saved = fs::read_to_string(&file).expect("palette written");
        assert!(stderr(&output).starts_with(&format!("motif: saved {}\n", file.display())));

        let dry = motif(out, &[&"import", &"--dry-run", &dir]);
        assert!(dry.status.success(), "{name}");
        assert_eq!(
            String::from_utf8(dry.stdout).expect("UTF-8"),
            saved,
            "{name}"
        );

        let doc: serde_json::Value = serde_json::from_str(&saved).expect("JSON");
        assert!(has_palette_shape(&saved, &doc), "{name}");
        assert_eq!(
            doc["meta"]["source"],
            serde_json::json!({ "kind": "omarchy-colors" }),
            "{name}"
        );
        count += 1;
    }
    assert_eq!(count, 23);
    assert_eq!(entries(out).len(), 23, "no temporary files left behind");
}

/// Checks the sections of a saved palette, their order and their sizes.
/// The library's tests check the rest of the schema.
fn has_palette_shape(text: &str, doc: &serde_json::Value) -> bool {
    let sections: Vec<usize> = [
        "\n  \"meta\": {",
        "\n  \"ansi\": {",
        "\n  \"colors\": {",
        "\n  \"palette\": {",
    ]
    .iter()
    .filter_map(|section| text.find(section))
    .collect();
    sections.len() == 4
        && sections.is_sorted()
        && doc["palette"].as_object().is_some_and(|p| p.len() == 63)
        && doc["ansi"].as_object().is_some_and(|a| a.len() == 17)
}

#[test]
fn dry_run_writes_nothing() {
    let tmp = TempDir::new().expect("temp dir");
    let output = motif(
        tmp.path(),
        &[&"import", &"--dry-run", &"-q", &theme("nord")],
    );
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(doc["meta"]["slug"], "nord");
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

#[test]
fn refuses_to_overwrite_without_force() {
    let tmp = TempDir::new().expect("temp dir");
    let out = tmp.path();
    let file = out.join("nord.json");
    fs::write(&file, "mine\n").expect("write");

    let output = motif(out, &[&"import", &"--output-dir", &out, &theme("nord")]);
    assert_eq!(output.status.code(), Some(1));
    // Only the error: nothing found in the theme is reported for a failed run.
    assert_eq!(
        stderr(&output),
        format!(
            "motif: {}: a palette with this slug exists; pass --force to replace it or --slug to choose another\n",
            file.display()
        )
    );
    assert_eq!(fs::read_to_string(&file).expect("read"), "mine\n");
    assert_eq!(entries(out), ["nord.json"]);

    let output = motif(
        out,
        &[
            &"import",
            &"--output-dir",
            &out,
            &"--force",
            &"-q",
            &theme("nord"),
        ],
    );
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert!(fs::read_to_string(&file).expect("read").starts_with('{'));
}

#[test]
fn saves_to_the_configuration_folder_by_default() {
    let tmp = TempDir::new().expect("temp dir");
    let palettes = tmp.path().join("matugen/palettes");
    fs::create_dir_all(&palettes).expect("create");
    let output = motif(
        tmp.path(),
        &[&"import", &"--slug", &"my-nord", &theme("nord")],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let file = palettes.join("my-nord.json");
    assert!(file.is_file());
    let message = stderr(&output);
    assert!(
        message.starts_with(&format!(
            "motif: saved {0}; to keep it on every machine, run `dots capture {0}` then `dots push`\n",
            file.display()
        )),
        "{message}"
    );
}

#[test]
fn falls_back_to_home_without_xdg_config_home() {
    let tmp = TempDir::new().expect("temp dir");
    let palettes = tmp.path().join(".config/matugen/palettes");
    fs::create_dir_all(&palettes).expect("create");
    let output = Command::new(env!("CARGO_BIN_EXE_motif"))
        .args(["import", "-q"])
        .arg(theme("white"))
        .env_clear()
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", "relative/ignored")
        .output()
        .expect("motif runs");
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(entries(&palettes), ["white.json"]);
}

#[test]
fn reports_a_missing_output_folder() {
    let tmp = TempDir::new().expect("temp dir");
    let output = motif(tmp.path(), &[&"import", &theme("nord")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("palettes is not a folder"),
        "{}",
        stderr(&output)
    );
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

#[test]
fn reports_a_bad_path() {
    let tmp = TempDir::new().expect("temp dir");
    let missing = tmp.path().join("missing");
    let output = motif(tmp.path(), &[&"import", &"--dry-run", &missing]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!(
            "motif: cannot read {}: No such file or directory (os error 2)\n",
            missing.display()
        )
    );
    assert_eq!(output.stdout, b"");

    let output = motif(tmp.path(), &[&"import", &"--dry-run", &tmp.path()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).ends_with("has no theme file motif can read\n"));
}

#[test]
fn reports_an_incomplete_theme_with_its_cause() {
    let tmp = TempDir::new().expect("temp dir");
    let dir = tmp.path().join("broken");
    fs::create_dir(&dir).expect("create");
    fs::write(dir.join("colors.toml"), "mode = \"dark\"\n").expect("write");
    let output = motif(tmp.path(), &[&"import", &"--dry-run", &dir]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).ends_with("does not describe a complete theme: background has no value\n"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn rejects_bad_slugs_as_usage_errors() {
    let tmp = TempDir::new().expect("temp dir");
    for slug in ["Nord", "../nord", "-nord", "wallpaper-derived", ""] {
        let flag = format!("--slug={slug}");
        let output = motif(tmp.path(), &[&"import", &flag, &theme("nord")]);
        assert_eq!(output.status.code(), Some(2), "{slug:?}");
        assert!(stderr(&output).contains("--slug"), "{slug:?}");
    }
    assert_eq!(entries(tmp.path()), Vec::<String>::new());
}

#[test]
fn reports_a_name_without_a_slug() {
    let tmp = TempDir::new().expect("temp dir");
    let output = motif(
        tmp.path(),
        &[&"import", &"--dry-run", &"--name", &"東京", &theme("nord")],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("pass --slug"));
}

#[test]
fn usage_errors_exit_with_status_2() {
    let tmp = TempDir::new().expect("temp dir");
    for args in [
        &[&"import" as &dyn AsRef<std::ffi::OsStr>][..],
        &[&"import", &"--semantic", &"loud", &theme("nord")],
        &[&"import", &"--name", &"", &theme("nord")],
        &[&"frobnicate"],
    ] {
        assert_eq!(motif(tmp.path(), args).status.code(), Some(2));
    }
    let help = motif(tmp.path(), &[&"import", &"--help"]);
    assert!(help.status.success());
    let version = motif(tmp.path(), &[&"--version"]);
    assert_eq!(
        version.stdout,
        format!("motif {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}

#[test]
fn prints_plain_text_when_not_a_terminal() {
    let tmp = TempDir::new().expect("temp dir");
    let output = motif(
        tmp.path(),
        &[&"import", &"--dry-run", &"-v", &theme("lumon")],
    );
    assert!(
        stderr(&output)
            .lines()
            .all(|l| l.starts_with("motif: warning: lumon: "))
    );
    assert!(!output.stderr.contains(&0x1b));
    let usage = motif(tmp.path(), &[&"import"]);
    assert!(!usage.stderr.contains(&0x1b));
}

#[test]
fn semantic_choice_changes_only_the_status_colors() {
    let tmp = TempDir::new().expect("temp dir");
    let run = |semantic: &str| {
        let output = motif(
            tmp.path(),
            &[
                &"import",
                &"--dry-run",
                &"--semantic",
                &semantic,
                &theme("lumon"),
            ],
        );
        assert!(output.status.success());
        serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("JSON")
    };
    let literal = run("literal");
    let fixed = run("fixed");
    assert_eq!(literal["palette"]["error"], "#4d86b0");
    assert_eq!(fixed["palette"]["error"], "#f4857b");
    assert_eq!(literal["palette"]["primary"], fixed["palette"]["primary"]);
    assert_eq!(literal["ansi"], fixed["ansi"]);
}

#[test]
fn counts_low_contrast_pairs_unless_verbose() {
    let tmp = TempDir::new().expect("temp dir");
    let out = tmp.path();
    let output = motif(out, &[&"import", &"--output-dir", &out, &theme("nord")]);
    assert!(output.status.success());
    let lines: Vec<String> = stderr(&output).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("motif: saved "));
    let count: usize = lines[1]
        .strip_prefix("motif: note: ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("a count");
    assert!(count > 1);
    assert!(
        lines[1].ends_with(" color pairs in nord are below WCAG contrast; --verbose lists them")
    );

    let verbose = motif(
        out,
        &[&"import", &"--dry-run", &"--verbose", &theme("nord")],
    );
    assert!(verbose.status.success());
    let listed: Vec<String> = stderr(&verbose).lines().map(str::to_owned).collect();
    assert_eq!(listed.len(), count);
    assert!(
        listed
            .iter()
            .all(|l| l.starts_with("motif: warning: nord: ") && l.contains(" has contrast "))
    );

    let dry = motif(out, &[&"import", &"--dry-run", &theme("nord")]);
    assert_eq!(stderr(&dry), format!("{}\n", lines[1]));

    let quiet = motif(out, &[&"import", &"--dry-run", &"-q", &theme("nord")]);
    assert_eq!(quiet.stderr, b"");
    let both = motif(
        out,
        &[&"import", &"--dry-run", &"-q", &"-v", &theme("nord")],
    );
    assert_eq!(both.status.code(), Some(2));
}

#[test]
fn reports_skipped_lines_after_the_save() {
    let tmp = TempDir::new().expect("temp dir");
    let dir = tmp.path().join("odd");
    fs::create_dir(&dir).expect("create");
    let mut colors = fs::read_to_string(theme("nord").join("colors.toml")).expect("read");
    colors.push_str("[colors]\n");
    let line = colors.lines().count();
    fs::write(dir.join("colors.toml"), &colors).expect("write");
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("create");

    let output = motif(tmp.path(), &[&"import", &"--output-dir", &out, &dir]);
    assert!(output.status.success());
    let lines: Vec<String> = stderr(&output).lines().map(str::to_owned).collect();
    assert!(lines[0].starts_with("motif: saved "), "{lines:?}");
    let expected = format!(
        "motif: warning: {}:{line}: key has unsupported characters; Omarchy ignores the line",
        fs::canonicalize(&dir)
            .expect("canonical")
            .join("colors.toml")
            .display()
    );
    assert_eq!(lines[1], expected);

    // A failed run reports the error alone.
    let again = motif(tmp.path(), &[&"import", &"--output-dir", &out, &dir]);
    assert_eq!(again.status.code(), Some(1));
    assert_eq!(stderr(&again).lines().count(), 1);
}
