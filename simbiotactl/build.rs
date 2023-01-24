use clap_complete::generate_to;
use clap_complete::Shell::{Bash, Fish, Zsh};
use std::env;
use std::io::Error;

use clap::CommandFactory;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    let outdir = match env::var("OUT_DIR") {
        Err(_) => panic!("cannot generate completion scripts without CARGO_TARGET_DIR"),
        Ok(outdir) => outdir,
    };

    let mut cmd = Cli::command();
    let path = generate_to(
        Bash,
        &mut cmd,       // We need to specify what generator to use
        "simbiotactl",  // We need to specify the bin name manually
        outdir.clone(), // We need to specify where to write to
    )?;
    println!("cargo:warning=bash completion file is generated: {path:?}");

    let path = generate_to(
        Zsh,
        &mut cmd,       // We need to specify what generator to use
        "simbiotactl",  // We need to specify the bin name manually
        outdir.clone(), // We need to specify where to write to
    )?;
    println!("cargo:warning=zsh completion file is generated: {path:?}");

    let path = generate_to(
        Fish,
        &mut cmd,      // We need to specify what generator to use
        "simbiotactl", // We need to specify the bin name manually
        outdir,        // We need to specify where to write to
    )?;
    println!("cargo:warning=fish completion file is generated: {path:?}");

    Ok(())
}
