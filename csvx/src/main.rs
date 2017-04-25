extern crate chrono;
extern crate clap;
#[macro_use]
extern crate lazy_static;
extern crate regex;
#[macro_use]
extern crate safe_unwrap;
use std::io;

use chrono::NaiveDate;
use clap::{App, Arg, SubCommand};
use regex::Regex;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CsvxMetadata {
    pub table_name: String,
    pub date: NaiveDate,
    pub schema: String,
    pub schema_version: usize,
    pub csvx_version: usize,
    pub is_schema: bool,
}

fn parse_filename<S: AsRef<str>>(filename: S) -> Option<CsvxMetadata> {
    lazy_static! {
        // `tablename_date_schema-schemaversion_csvxversion.csvx`
        static ref FN_RE: Regex = Regex::new(r"^([a-z][a-z0-9_].*)-\d.*$")
        .expect("built-in Regex is broken. Please file a bug");
    }
    println!("Input: {:?}", filename.as_ref());
    println!("Result: {:?}", FN_RE.find(filename.as_ref()));

    unimplemented!()
}

fn main() {
    let app = App::new("csvx")
        .version("2.0.0")
        .about("csvx version 2 utility")
        .subcommand(SubCommand::with_name("check")
                        .about("Check csvx files for conformance")
                        .arg(Arg::with_name("schema_file")
                                 .help("Schema file to check against")
                                 .required(true)
                                 .takes_value(true))
                        .arg(Arg::with_name("input_files")
                                 .help("Input files to check")
                                 .multiple(true)
                                 .takes_value(true)));
    let m = app.clone().get_matches();

    match m.subcommand {
        Some(ref cmd) if cmd.name == "check" => {
            let schema_file = safe_unwrap!("required argument",
                                           cmd.matches.value_of("schema_file"));
            // first, check filename of schema file
            parse_filename(schema_file);
            // println!("{:?}", m);
        }
        _ => app.write_help(&mut io::stdout()).unwrap(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use chrono::NaiveDate;

    #[test]
    fn filename_parsing_rejects_invalid() {
        assert_eq!(parse_filename("asdf"), None);
        assert_eq!(parse_filename(""), None);
        assert_eq!(parse_filename("test.csv"), None);
        assert_eq!(parse_filename("test.csv"), None);
    }

    #[test]
    fn filename_parsing_parses_valid() {
        assert_eq!(parse_filename("zoo-nyc_20170401_animals-2_3.csv").unwrap(),
                   CsvxMetadata {
                       table_name: "zoo-nyc".to_owned(),
                       date: NaiveDate::from_ymd(2017, 04, 01),
                       schema: "animals".to_owned(),
                       schema_version: 2,
                       csvx_version: 3,
                       is_schema: false,
                   });
    }

}
