//! Compile reviewed text grammars once at build time, outside the input loop.

use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use syntect::parsing::{SyntaxDefinition, SyntaxSetBuilder};

#[path = "build_support/revision.rs"]
mod revision;

fn main() -> Result<()> {
    record_revision()?;
    println!("cargo:rerun-if-changed=assets/syntax");
    let directory =
        std::env::var_os("OUT_DIR").context("Cargo must provide OUT_DIR")?;
    let mut builder = SyntaxSetBuilder::new();
    builder.add_plain_text_syntax();
    for source in GRAMMARS {
        builder.add(SyntaxDefinition::load_from_str(source, true, None)?);
    }
    let output = PathBuf::from(directory).join("reader-syntaxes.pack");
    let syntaxes = builder.build();
    let unresolved = syntaxes.find_unlinked_contexts();
    ensure!(
        unresolved.is_empty(),
        "Unlinked grammar contexts: {unresolved:?}"
    );
    syntect::dumps::dump_to_file(&syntaxes, output)?;
    Ok(())
}

fn record_revision() -> Result<()> {
    println!("cargo:rerun-if-env-changed=LUME_BUILD_REVISION");
    let directory = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .context("Cargo must provide CARGO_MANIFEST_DIR")?;
    let supplied = std::env::var("LUME_BUILD_REVISION").ok();
    let metadata = revision::inspect(&directory, supplied.as_deref())?;
    for path in metadata.watched {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rustc-env=LUME_BUILD_REVISION={}", metadata.revision);
    Ok(())
}

const GRAMMARS: &[&str] = &[
    include_str!("assets/syntax/C.sublime-syntax"),
    include_str!("assets/syntax/C++.sublime-syntax"),
    include_str!("assets/syntax/Java.sublime-syntax"),
    include_str!("assets/syntax/JavaDoc.sublime-syntax"),
    include_str!("assets/syntax/JavaScript.sublime-syntax"),
    include_str!(
        "assets/syntax/Regular Expressions (JavaScript).sublime-syntax"
    ),
    include_str!("assets/syntax/TypeScript.sublime-syntax"),
    include_str!("assets/syntax/Terraform.sublime-syntax"),
    include_str!("assets/syntax/HTML.sublime-syntax"),
    include_str!("assets/syntax/CSS.sublime-syntax"),
    include_str!("assets/syntax/YAML.sublime-syntax"),
    include_str!("assets/syntax/Rust.sublime-syntax"),
    include_str!("assets/syntax/Go.sublime-syntax"),
    include_str!("assets/syntax/Python.sublime-syntax"),
    include_str!("assets/syntax/Regular Expressions (Python).sublime-syntax"),
    include_str!("assets/syntax/SQL.sublime-syntax"),
    include_str!("assets/syntax/LumeSQL.sublime-syntax"),
    include_str!("assets/syntax/RegExp.sublime-syntax"),
    include_str!("assets/syntax/JSON.sublime-syntax"),
    include_str!("assets/syntax/Bash.sublime-syntax"),
    include_str!("assets/syntax/Shell-Unix-Generic.sublime-syntax"),
    include_str!("assets/syntax/commands-builtin-shell-bash.sublime-syntax"),
];
