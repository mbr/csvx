extern crate chrono;
extern crate clap;
extern crate csv;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate safe_unwrap;
extern crate try_from;

use chrono::NaiveDate;
use clap::{App, Arg, SubCommand};
use std::{io, path};
use regex::Regex;
use safe_unwrap::SafeUnwrap;
use try_from::TryFrom;

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

lazy_static! {
    static ref CONSTRAINT_RE: Regex = Regex::new(
        r"^(:?(?:NULLABLE|UNIQUE),?)*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CsvxMetadata {
    pub table_name: String,
    pub date: NaiveDate,
    pub schema: String,
    pub csvx_version: usize,
}

impl CsvxMetadata {
    fn is_schema(&self) -> bool {
        self.schema == "csvx-schema"
    }
}

#[derive(Debug)]
enum SchemaLoadError {
    Csv(csv::Error),
    MissingHeader,
    BadHeader,
    BadIdentifier(usize, String),
    BadType(usize, ColumnTypeError),
    BadConstraints(usize, ColumnConstraintsError),
}

#[derive(Clone, Debug)]
enum ColumnTypeError {
    UnknownType,
}

#[derive(Clone, Debug)]
enum ColumnType {
    String,
    Bool,
    Integer,
    Enum(Vec<String>),
    Decimal,
    Date,
    DateTime,
    Time,
}

#[derive(Clone, Debug)]
struct ColumnConstraints {
    nullable: bool,
    unique: bool,
}

impl Default for ColumnConstraints {
    fn default() -> ColumnConstraints {
        ColumnConstraints {
            nullable: false,
            unique: false,
        }
    }
}

#[derive(Clone, Debug)]
enum ColumnConstraintsError {
    MalformedConstraint,
    UnknownConstraint(String),
}

impl<S> TryFrom<S> for ColumnConstraints
    where S: AsRef<str>
{
    type Err = ColumnConstraintsError;

    fn try_from(s: S) -> Result<ColumnConstraints, Self::Err> {
        if !CONSTRAINT_RE.is_match(s.as_ref()) {
            return Err(ColumnConstraintsError::MalformedConstraint);
        }

        let mut ccs = ColumnConstraints::default();

        if s.as_ref() == "" {
            return Ok(ccs);
        }

        for fragment in s.as_ref().split(',') {
            match s.as_ref() {
                "NULLABLE" => {
                    ccs.nullable = true;
                }
                "UNIQUE" => {
                    ccs.unique = true;
                }
                _ => return Err(ColumnConstraintsError::UnknownConstraint(s.as_ref().to_string())),
            }

        }

        Ok(ccs)
    }
}

impl<S> TryFrom<S> for ColumnType
    where S: AsRef<str>
{
    type Err = ColumnTypeError;

    fn try_from(s: S) -> Result<ColumnType, Self::Err> {
        match s.as_ref() {
            "STRING" => Ok(ColumnType::String),
            "BOOL" => Ok(ColumnType::Bool),
            "INTEGER" => Ok(ColumnType::Integer),
            "DECIMAL" => Ok(ColumnType::Decimal),
            "DATE" => Ok(ColumnType::Date),
            "DATETIME" => Ok(ColumnType::DateTime),
            "TIME" => Ok(ColumnType::Time),
            _ if ENUM_EXPR_RE.is_match(s.as_ref()) => {
                let variants: Vec<_> = ENUM_EXPR_RE
                    .captures(s.as_ref())
                    .safe_unwrap("match already exists")
                    .get(1)
                    .safe_unwrap("group 1 exists in regex")
                    .as_str()
                    .split(',')
                    .map(|s| s.to_owned())
                    .collect();

                Ok(ColumnType::Enum(variants))
            }
            _ => return Err(ColumnTypeError::UnknownType),

        }
    }
}

#[derive(Clone, Debug)]
struct CsvxColumnType {
    id: String,
    ty: ColumnType,
    constraints: ColumnConstraints,
    description: String,
}

#[derive(Clone, Debug)]
struct CsvxSchema {
    columns: Vec<CsvxColumnType>,
}

impl From<csv::Error> for SchemaLoadError {
    fn from(e: csv::Error) -> SchemaLoadError {
        SchemaLoadError::Csv(e)
    }
}

impl CsvxSchema {
    fn from_file<P: AsRef<path::Path>>(filename: P) -> Result<CsvxSchema, SchemaLoadError> {
        let mut rdr = csv::Reader::from_file(filename)?.has_headers(false);

        let mut it = rdr.decode();
        let header: Option<Result<(String, String, String, String), _>> = it.next();

        let mut columns = Vec::new();

        match header {
            None => return Err(SchemaLoadError::MissingHeader),
            Some(res) => {
                let fields = res?;

                println!("{:?}", fields);

                if fields.0 != "id" || fields.1 != "type" || fields.2 != "constraints" ||
                   fields.3 != "description" {

                    return Err(SchemaLoadError::BadHeader);
                }

                for (recno, rec) in it.enumerate() {
                    let (id, ty, constraints, desc) = rec?;
                    let lineno = recno + 2;

                    // check identifier
                    if !IDENT_UNDERSCORE_RE.is_match(&id.as_str()) {
                        return Err(SchemaLoadError::BadIdentifier(lineno, id));
                    }

                    // create type
                    let col_type = match ColumnType::try_from(ty.as_str()) {
                        Ok(v) => v,
                        Err(e) => return Err(SchemaLoadError::BadType(lineno, e)),
                    };

                    // create constraints
                    let col_constraints = match ColumnConstraints::try_from(constraints.as_str()) {
                        Ok(v) => v,
                        Err(e) => return Err(SchemaLoadError::BadConstraints(lineno, e)),
                    };

                    let col = CsvxColumnType {
                        id: id,
                        ty: col_type,
                        constraints: col_constraints,
                        description: desc,
                    };

                    columns.push(col)
                }

                Ok(CsvxSchema { columns: columns })
            }
        }

    }
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

            if !meta.is_schema() {
                println!("The supplied file {} is not a csvx schema (wrong filename)",
                         schema_file);
                return;
            }

            // load schema
            let schema = CsvxSchema::from_file(schema_file).unwrap();

            println!("{:?}", schema);
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
