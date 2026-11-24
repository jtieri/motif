//! `motif import`: turns a local theme folder into a palette file.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::ValueEnum;
use clap::builder::NonEmptyStringValueParser;
use motif_core::store::{self, Overwrite, WriteError};
use motif_core::validate::{ContrastWarning, Problem};
use motif_core::{LoadError, Options, Palette, Semantic, Slug, SlugError, Warning, source};

/// Import a theme folder as a palette.
///
/// Reads the theme in PATH, builds a palette from it and saves the palette
/// as SLUG.json in the palette folder, where the theme switcher finds it.
/// An existing palette is never replaced without --force.
#[derive(Debug, clap::Args)]
#[command(after_help = "\
Examples:
  motif import ~/src/omarchy-nord-theme
  motif import --name 'Nord Frost' --semantic fixed ./nord
  motif import --dry-run ./nord | jq .palette")]
pub(crate) struct Args {
    /// The theme folder, such as an Omarchy theme with a colors.toml.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// The name shown in the theme picker [default: the theme's name]
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    name: Option<String>,

    /// The palette's file name, without .json [default: made from the name]
    ///
    /// Lowercase letters, digits and hyphens, starting with a letter or
    /// digit, at most 64 characters.
    #[arg(long, value_parser = parse_slug)]
    slug: Option<Slug>,

    /// How to choose the error, warning and success colors.
    #[arg(long, value_enum, default_value_t = SemanticArg::Literal)]
    semantic: SemanticArg,

    #[arg(
        long,
        value_name = "DIR",
        help = "The folder to save the palette in [default: $XDG_CONFIG_HOME/matugen/palettes]"
    )]
    output_dir: Option<PathBuf>,

    /// Print the palette to standard output and save nothing.
    #[arg(long)]
    dry_run: bool,

    /// Replace a palette with the same slug.
    #[arg(long)]
    force: bool,

    #[command(flatten)]
    output: Verbosity,
}

/// How much an import prints besides errors.
#[derive(Debug, clap::Args)]
struct Verbosity {
    /// Print nothing but errors.
    #[arg(short, long)]
    quiet: bool,

    /// List each pair of colors below WCAG contrast, instead of a count.
    ///
    /// Low contrast is reported, never fixed: an imported theme keeps the
    /// colors its author chose.
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SemanticArg {
    /// The theme's own red, yellow and green.
    Literal,
    /// Canonical hues at the lightness and saturation of the theme's colors.
    Tinted,
    /// Canonical colors, whatever the theme.
    Fixed,
}

impl From<SemanticArg> for Semantic {
    fn from(arg: SemanticArg) -> Self {
        match arg {
            SemanticArg::Literal => Self::Literal,
            SemanticArg::Tinted => Self::Tinted,
            SemanticArg::Fixed => Self::Fixed,
        }
    }
}

fn parse_slug(s: &str) -> Result<Slug, String> {
    s.parse::<Slug>().map_err(|err| err.kind().to_string())
}

/// The error returned when an import fails.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ImportError {
    #[error("cannot read {}", path.display())]
    Path {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is not a folder", path.display())]
    NotAFolder { path: PathBuf },
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error("cannot make a slug from the name {name:?} because {}; pass --slug", error.kind())]
    Slug { name: String, error: SlugError },
    #[error("the palette made from {} is invalid, which is a bug in motif: {}", path.display(), list(problems))]
    Invalid {
        path: PathBuf,
        problems: Vec<Problem>,
    },
    #[error(
        "cannot find the palette folder: neither XDG_CONFIG_HOME nor HOME is an absolute path; pass --output-dir"
    )]
    NoConfigHome,
    #[error("cannot save the palette")]
    Write(#[source] WriteError),
    #[error("{}: a palette with this slug exists; pass --force to replace it or --slug to choose another", path.display())]
    Exists { path: PathBuf },
    #[error("cannot write the palette to standard output")]
    Stdout(#[source] io::Error),
}

fn list(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Runs `motif import`.
pub(crate) fn run(args: &Args) -> Result<(), ImportError> {
    let output_dir = match (&args.output_dir, args.dry_run) {
        (Some(dir), _) => Some(dir.clone()),
        (None, false) => Some(default_output_dir().ok_or(ImportError::NoConfigHome)?),
        (None, true) => None,
    };

    let path = fs::canonicalize(&args.path).map_err(|source| ImportError::Path {
        path: args.path.clone(),
        source,
    })?;
    if !path.is_dir() {
        return Err(ImportError::NotAFolder { path });
    }

    let loaded = source::load(&path)?;
    let options = Options {
        name: args.name.clone(),
        slug: args.slug.clone(),
        semantic: args.semantic.into(),
    };
    let palette =
        Palette::from_theme(&loaded.theme, &options).map_err(|error| ImportError::Slug {
            name: options
                .name
                .clone()
                .unwrap_or_else(|| loaded.theme.name.clone()),
            error,
        })?;

    let report = palette.validate();
    if !report.is_valid() {
        return Err(ImportError::Invalid {
            path,
            problems: report.problems,
        });
    }
    let json = palette.to_json();
    let Some(dir) = output_dir else {
        print(&json)?;
        if !args.output.quiet {
            findings(args, &loaded.warnings, &report.warnings, &palette);
        }
        return Ok(());
    };
    let overwrite = if args.force {
        Overwrite::Allow
    } else {
        Overwrite::Never
    };
    let file_name = format!("{}.json", palette.meta.slug);
    let written = store::write_atomic(&dir, &file_name, json.as_bytes(), overwrite).map_err(
        |err| match err {
            WriteError::Exists { path } => ImportError::Exists { path },
            other => ImportError::Write(other),
        },
    )?;

    if !args.output.quiet {
        let message = if args.output_dir.is_none() {
            format!(
                "motif: saved {}; to keep it on every machine, run `dots capture {0}` then `dots push`",
                written.display()
            )
        } else {
            format!("motif: saved {}", written.display())
        };
        crate::diagnostic(&message);
        findings(args, &loaded.warnings, &report.warnings, &palette);
    }
    Ok(())
}

/// Prints what a successful import found: each problem in the theme file,
/// then the pairs below WCAG contrast, listed with --verbose and otherwise
/// counted on one line.
fn findings(args: &Args, skipped: &[Warning], contrast: &[ContrastWarning], palette: &Palette) {
    for warning in skipped {
        crate::warning(warning);
    }
    let slug = &palette.meta.slug;
    if args.output.verbose {
        for warning in contrast {
            crate::warning(&format_args!("{slug}: {warning}"));
        }
    } else if !contrast.is_empty() {
        let (pairs, verb) = if contrast.len() == 1 {
            ("pair", "is")
        } else {
            ("pairs", "are")
        };
        crate::diagnostic(&format!(
            "motif: note: {} color {pairs} in {slug} {verb} below WCAG contrast; --verbose lists them",
            contrast.len()
        ));
    }
}

/// Returns `$XDG_CONFIG_HOME/matugen/palettes`, or `~/.config/matugen/palettes`
/// when `XDG_CONFIG_HOME` is unset or not absolute, as the XDG Base Directory
/// specification requires.
fn default_output_dir() -> Option<PathBuf> {
    let absolute = |var| {
        env::var_os(var)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let config = absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))?;
    Some(config.join("matugen").join("palettes"))
}

/// Writes the palette to standard output. A reader that stops early, as
/// `head` does, is not an error.
fn print(json: &str) -> Result<(), ImportError> {
    let mut stdout = io::stdout().lock();
    match stdout
        .write_all(json.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(err) if err.kind() != io::ErrorKind::BrokenPipe => Err(ImportError::Stdout(err)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_problems() {
        let problems = [
            Problem {
                key: "a".into(),
                message: "is missing".into(),
            },
            Problem {
                key: "b".into(),
                message: "is not a #rrggbb color".into(),
            },
        ];
        assert_eq!(list(&problems), "a: is missing; b: is not a #rrggbb color");
    }

    #[test]
    fn rejects_bad_slugs_on_the_command_line() {
        assert!(parse_slug("ok-1").is_ok());
        assert_eq!(
            parse_slug("Bad").unwrap_err(),
            "only a-z, 0-9 and - are allowed"
        );
    }

    #[test]
    fn checks_the_command_line_definition() {
        use clap::CommandFactory;
        crate::Cli::command().debug_assert();
    }
}
