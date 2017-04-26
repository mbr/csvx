extern crate chrono;
extern crate clap;
extern crate csv;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate safe_unwrap;
extern crate try_from;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::{App, Arg, SubCommand};
use std::{io, path};
use regex::Regex;
use safe_unwrap::SafeUnwrap;
use try_from::TryFrom;

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

lazy_static! {
    static ref DECIMAL_RE: Regex = Regex::new(
        r"^\d+(?:\.\d+)?$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref DATE_RE: Regex = Regex::new(
        r"^(\d{4})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref DATETIME_RE: Regex = Regex::new(
        r"^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    static ref TIME_RE: Regex = Regex::new(
        r"^(\d{2})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    // `tablename_date_schema-schemaversion_csvxversion.csvx`
    static ref FN_RE: Regex = Regex::new(
        r"^([a-z][a-z0-9-]*)_(\d{4})(\d{2})(\d{2})_([a-z][a-z0-9-]*)_(\d+).csv$"
    ).expect("built-in Regex is broken. Please file a bug");
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

#[derive(Debug)]
enum ValidationError {
    Csv(csv::Error),
    MissingHeaders,
    HeaderMismatch(usize, String),
    RowLengthMismatch(usize),
    ValueError(usize, usize, ValueError),
}

impl From<csv::Error> for ValidationError {
    fn from(e: csv::Error) -> ValidationError {
        ValidationError::Csv(e)
    }
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
            match fragment.as_ref() {
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

#[derive(Debug)]
enum Value {
    String(String),
    Bool(bool),
    Integer(i64),
    Enum(String),
    Decimal(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Time(NaiveTime),
}

#[derive(Debug)]
enum ValueError {
    NonNullable,
    InvalidBool,
    InvalidInt,
    InvalidEnum,
    InvalidDecimal,
    InvalidDate,
    InvalidDateTime,
    InvalidTime,
}

impl From<std::num::ParseIntError> for ValueError {
    fn from(_: std::num::ParseIntError) -> ValueError {
        ValueError::InvalidInt
    }
}

impl CsvxColumnType {
    fn validate_value<S: AsRef<str>>(&self, s: &S) -> Result<Option<Value>, ValueError> {
        // FIXME: check UNIQUE

        // null check
        if s.as_ref() == "" {
            if self.constraints.nullable {
                return Ok(None);
            } else {
                return Err(ValueError::NonNullable);
            }
        }

        match self.ty {
            ColumnType::String => Ok(Some(Value::String(s.as_ref().to_string()))),
            ColumnType::Bool => {
                match s.as_ref() {
                    "TRUE" => Ok(Some(Value::Bool(true))),
                    "FALSE" => Ok(Some(Value::Bool(false))),
                    _ => Err(ValueError::InvalidBool),
                }
            }
            ColumnType::Integer => {
                // FIXME: check for leading zeros
                Ok(Some(Value::Integer(s.as_ref().parse()?)))
            }
            ColumnType::Enum(ref variants) => {
                let v = s.as_ref().to_owned();
                if variants.contains(&v) {
                    Ok(Some(Value::Enum(v)))
                } else {
                    Err(ValueError::InvalidEnum)
                }
            }
            ColumnType::Decimal => {
                if !DECIMAL_RE.is_match(s.as_ref()) {
                    Ok(Some(Value::Decimal(s.as_ref().to_owned())))
                } else {
                    Err(ValueError::InvalidDecimal)
                }
            }
            ColumnType::Date => {
                match DATE_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        Ok(Some(Value::Date(NaiveDate::from_ymd_opt(cap(c, 1),
                                                                    cap(c, 2),
                                                                    cap(c, 3))
                                                    .ok_or(ValueError::InvalidDate)?)))
                    }
                    None => Err(ValueError::InvalidDate),
                }
            }
            ColumnType::DateTime => {
                match DATETIME_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        let dt = NaiveDate::from_ymd_opt(cap(c, 1), cap(c, 2), cap(c, 3))
                            .ok_or(ValueError::InvalidDate)?;
                        Ok(Some(Value::DateTime(dt.and_hms_opt(cap(c, 4), cap(c, 5), cap(c, 6))
                                                    .ok_or(ValueError::InvalidTime)?)))
                    }
                    None => Err(ValueError::InvalidDateTime),
                }
            }
            ColumnType::Time => {
                match TIME_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        Ok(Some(Value::Time(NaiveTime::from_hms_opt(cap(c, 1),
                                                                    cap(c, 2),
                                                                    cap(c, 3))
                                                    .ok_or(ValueError::InvalidTime)?)))
                    }
                    None => Err(ValueError::InvalidTime),
                }
            }
        }
    }
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

    fn validate_file<P: AsRef<path::Path>>(&self, filename: P) -> Result<(), ValidationError> {
        let mut rdr = csv::Reader::from_file(filename)?.has_headers(true);

        let headers = rdr.headers()?;

        if headers.len() != self.columns.len() {
            return Err(ValidationError::MissingHeaders);
        }

        for (idx, (spec, actual)) in self.columns.iter().zip(headers.iter()).enumerate() {
            if spec.id.as_str() != actual {
                return Err(ValidationError::HeaderMismatch(idx + 1, actual.to_string()));
            }
        }

        for (rowid, row) in rdr.records().enumerate() {
            let lineno = rowid + 2;

            let fields = row?;

            if fields.len() != self.columns.len() {
                return Err(ValidationError::RowLengthMismatch(lineno));
            }

            for (idx, (col, value)) in self.columns.iter().zip(fields.iter()).enumerate() {
                if let Err(e) = col.validate_value(value) {
                    let col_idx = idx + 1;
                    println!("Value Error in line {}, column {}. Value: {}.
                        Error: {:?}",
                             lineno,
                             col_idx,
                             value,
                             e);
                    return Err(ValidationError::ValueError(lineno, col_idx, e));
                }
            }
        }

        Ok(())
    }
}

#[inline]
fn cap<T>(c: &regex::Captures, idx: usize) -> T
    where T: std::str::FromStr,
          T::Err: std::fmt::Debug
{
    c.get(idx)
        .safe_unwrap("valid group")
        .as_str()
        .parse()
        .safe_unwrap("already validated through regex")

}

fn parse_filename<S: AsRef<str>>(filename: S) -> Option<CsvxMetadata> {
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
                     date: match NaiveDate::from_ymd_opt(year, month, day) {
                         Some(d) => d,
                         None => return None,
                     },
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

            let meta_fn = path::Path::new(schema_file)
                .file_name()
                .expect("Not a valid filename")
                .to_str()
                .safe_unwrap("From valid UTF8");
            let meta = parse_filename(meta_fn).expect("schema filename is not
                in valid format");

            if !meta.is_schema() {
                println!("The supplied file {} is not a csvx schema (wrong filename)",
                         schema_file);
                return;
            }

            // load schema
            let schema = CsvxSchema::from_file(schema_file).unwrap();

            let input_files = cmd.matches.values_of("input_files");

            if let Some(ifs) = input_files {
                for input_file in ifs {
                    println!("Validating {}", input_file);
                    schema.validate_file(input_file).unwrap();
                }
            }
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
