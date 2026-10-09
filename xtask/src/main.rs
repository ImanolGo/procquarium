//! Generate the man page and shell completions from the clap definition.
//!
//! Run with `cargo run -p xtask`. The output is committed so the release
//! archives and the `.deb` can pick it up without running Rust code at package
//! time; CI regenerates it and fails if the committed copy is stale.

use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::{Shell, generate_to};
use procquarium::cli::Cli;

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the repo")
        .to_path_buf();
    let man_dir = root.join("man");
    let completions_dir = root.join("completions");
    std::fs::create_dir_all(&man_dir)?;
    std::fs::create_dir_all(&completions_dir)?;

    let mut command = Cli::command();

    let man_path = man_dir.join("procquarium.1");
    let mut file =
        File::create(&man_path).with_context(|| format!("creating {}", man_path.display()))?;
    clap_mangen::Man::new(command.clone())
        .render(&mut file)
        .with_context(|| format!("writing {}", man_path.display()))?;

    for (name, shell) in [
        ("bash", Shell::Bash),
        ("zsh", Shell::Zsh),
        ("fish", Shell::Fish),
    ] {
        generate_to(shell, &mut command, "procquarium", &completions_dir)
            .with_context(|| format!("writing the {name} completion"))?;
    }

    Ok(())
}
