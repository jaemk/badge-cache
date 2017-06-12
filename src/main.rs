extern crate badge_cache;
#[macro_use] extern crate clap;

use std::env;
use badge_cache::service;
use badge_cache::errors::*;
use badge_cache::admin;

use clap::{Arg, App, SubCommand, ArgMatches};

pub fn main() {
    let matches = App::new("badge-cache")
        .version(crate_version!())
        .about("Shields IO Badge Caching Server")
        .subcommand(SubCommand::with_name("serve")
                    .about("Initialize Server")
                    .arg(Arg::with_name("port")
                         .long("port")
                         .short("p")
                         .takes_value(true)
                         .help("Port to listen on. Defaults to 3000"))
                    .arg(Arg::with_name("public")
                         .long("public")
                         .help("Serve on '0.0.0.0' instead of 'localhost'"))
                    .arg(Arg::with_name("log")
                         .long("log")
                         .help("Output logging info. Shortcut for settings env-var LOG=info")))
        .subcommand(SubCommand::with_name("admin")
                    .about("admin functions")
                    .arg(Arg::with_name("badge-dir")
                         .long("clear-cached-badges")
                         .takes_value(true)
                         .help("Clear out any cached badges in the given dir.\nProvide a blank string, '', to append `static/badges` to the current dir."))
                    .arg(Arg::with_name("no-confirm")
                         .long("no-confirm")
                         .takes_value(false)
                         .help("Auto-confirm/skip any confirmation checks")))
        .get_matches();

    if let Err(error) = run(&matches) {
        println!("Error: {}", error);
        ::std::process::exit(1);
    }
}


fn run(matches: &ArgMatches) -> Result<()> {
    if let Some(serve_matches) = matches.subcommand_matches("serve") {
        if serve_matches.is_present("log") {
            env::set_var("LOG", "info");
        }
        let port = serve_matches.value_of("port").unwrap_or("3000");
        let host_base = if serve_matches.is_present("public") { "0.0.0.0" } else { "localhost" };
        let host = format!("{}:{}", host_base, port);
        service::start(&host);
        return Ok(());
    }

    if let Some(admin_matches) = matches.subcommand_matches("admin") {
        return admin::handle(admin_matches)
    }

    println!("badge-cache: see `--help`");
    Ok(())
}


