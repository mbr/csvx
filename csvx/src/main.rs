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
use safe_unwrap::SafeUnwrap;

lazy_static! {
    static ref IDENT_RE: Regex = Regex::new(
        r"^[a-z][a-z0-9]*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref IDENT_HYPHEN_RE: Regex = Regex::new(
        r"^[a-z][a-z0-9-]*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref IDENT_UNDERSCORE_RE: Regex = Regex::new(
        r"^[a-z][a-z0-9_]*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref ENUM_EXPR_RE: Regex = Regex::new(
        r"^ENUM.*\(((?:[A-Z][A-Z0-9]*,?)*)\)$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CsvxMetadata {
    pub table_name: String,
    pub date: NaiveDate,
    pub schema: String,
    pub csvx_version: usize,
}

#[inline]
fn cap<T>(c: &regex::Captures, idx: usize) -> T
    where T: std::str::FromStr,
          T::Err: std::fmt::Debug
{
    println!("TRYING: {:?}", c.get(idx));

    c.get(idx)
        .safe_unwrap("valid group")
        .as_str()
        .parse()
        .safe_unwrap("already validated through regex")

}

fn parse_filename<S: AsRef<str>>(filename: S) -> Option<CsvxMetadata> {
    lazy_static! {
        // `tablename_date_schema-schemaversion_csvxversion.csvx`
        static ref FN_RE: Regex = Regex::new(
            r"^([a-z][a-z0-9-]*)_(\d{4})(\d{2})(\d{2})_([a-z][a-z0-9-]*)_(\d+).csv$"
        ).expect("built-in Regex is broken. Please file a bug");
    }

    match FN_RE.captures(filename.as_ref()) {
        Some(caps) => {
            let table_name = caps.get(1)
                .safe_unwrap("known group")
                .as_str()
                .to_string();
            let year = cap(&caps, 2);
            let month = cap(&caps, 3);
            let day = cap(&caps, 4);
            let schema = caps.get(5)
                .safe_unwrap("known group")
                .as_str()
                .to_string();
            let csvx_version = cap(&caps, 6);

            Some(CsvxMetadata {
                     table_name: table_name,
                     date: NaiveDate::from_ymd(year, month, day),
                     schema: schema,
                     csvx_version: csvx_version,
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
            let schema_file = cmd.matches
                .value_of("schema_file")
                .safe_unwrap("required argument");

            let meta = parse_filename(schema_file).expect("filename is not in valid format");
            println!("{:?}", meta);
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
                       schema: "animals-2".to_owned(),
                       csvx_version: 3,
                   });
    }

}
