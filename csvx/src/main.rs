extern crate clap;

use clap::{App, Arg, SubCommand};
use std::io;

fn main() {
    let app = App::new("csvx")
        .version("2.0.0")
        .about("csvx version 2 utility")
        .subcommand(SubCommand::with_name("check")
                        .about("Check csvx files for conformance")
                        .arg(Arg::with_name("schema_file")
                                 .help("Schema file to check against")
                                 .takes_value(true))
                        .arg(Arg::with_name("input_files")
                                 .help("Input files to check")
                                 .multiple(true)
                                 .takes_value(true)));
    let m = app.clone().get_matches();

    match m.subcommand {
        Some(ref cmd) if cmd.name == "check" => {
            println!("{:?}", m);

            unimplemented!()
        }
        _ => app.write_help(&mut io::stdout()).unwrap(),
    }
}
