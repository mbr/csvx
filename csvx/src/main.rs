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

#[inline]
fn cap<T>(c: &regex::Captures, idx: usize) -> T
    where T: std::str::FromStr,
          T::Err: std::fmt::Debug
{
    println!("TRYING: {:?}", c.get(idx));

    safe_unwrap!("input validated via regular expression",
                 safe_unwrap!("valid regex group index", c.get(idx))
                     .as_str()
                     .parse())

}

fn parse_filename<S: AsRef<str>>(filename: S) -> Option<CsvxMetadata> {
    lazy_static! {
        // `tablename_date_schema-schemaversion_csvxversion.csvx`
        static ref FN_RE: Regex = Regex::new(
            r"^([a-z][a-zA-Z0-9-]*)_(\d{4})(\d{2})(\d{2})_([a-z][a-zA-Z0-9-]*)-(\d+)_(\d+).csv(x?)$"
        ).expect("built-in Regex is broken. Please file a bug");
    }

    println!("Input: {:?}", filename.as_ref());
    println!("Result: {:?}", FN_RE.find(filename.as_ref()));

    match FN_RE.captures(filename.as_ref()) {
        Some(caps) => {
            let table_name = safe_unwrap!("known group", caps.get(1))
                .as_str()
                .to_string();
            let year = cap(&caps, 2);
            let month = cap(&caps, 3);
            let day = cap(&caps, 4);
            let schema = safe_unwrap!("known group", caps.get(5))
                .as_str()
                .to_string();
            let schema_version = cap(&caps, 6);
            let csvx_version = cap(&caps, 7);
            let trailing_x = safe_unwrap!("known group", caps.get(8));

            Some(CsvxMetadata {
                     table_name: table_name,
                     date: NaiveDate::from_ymd(year, month, day),
                     schema: schema,
                     schema_version: schema_version,
                     csvx_version: csvx_version,
                     is_schema: trailing_x.start() != trailing_x.end(),
                 })
        }
        None => None,
    }
}

fn main() {
    let app = App::new("csvx")
        .version("4.0.0")
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
