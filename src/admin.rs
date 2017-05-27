/*!
General Admin Commands
*/

use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::Write;
use clap::ArgMatches;

use errors::*;


fn default_static_root() -> PathBuf {
    let mut root = env::current_dir().expect("Failed to get the current directory");
    root.push("static/badges");
    root
}


/// Print a message and require y/n confirmation
fn confirm(msg: &str) -> Result<()> {
    print!("{}", msg);
    ::std::io::stdout().flush().expect("Error flushing stdout");
    let mut input = String::new();
    let stdin = ::std::io::stdin();
    stdin.read_line(&mut input).expect("Error reading stdin");
    if input.trim().to_lowercase() == "y" { return Ok(()) }
    Err(Error::Msg("Unable to confirm...".to_string()))
}


fn clear_cached_files(no_confirm: bool, dir: &str) -> Result<()> {
    let static_root = if dir.is_empty() { default_static_root() } else { PathBuf::from(dir) };
    if !no_confirm {
        confirm(&format!("** Delete everything in {:?}? (y/n) > ", &static_root))?;
    }
    let read_dir = fs::read_dir(&static_root)
        .map_err(|e| Error::IoErrorMsg(e, format!("Unable to read `STATIC_ROOT` dir: {:?} - make sure you run this from the project root", &static_root)))?;
    for entry in read_dir {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        };
    }
    println!("[badge-cache] [admin] - cleaned out cached badges in {:?}", &static_root);
    Ok(())
}


pub fn handle(matches: &ArgMatches) -> Result<()> {
    let no_confirm = matches.is_present("no-confirm");
    if let Some(dir) = matches.value_of("badge-dir") {
        clear_cached_files(no_confirm, &dir)?;
        return Ok(())
    }
    Ok(())
}
