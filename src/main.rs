extern crate chrono;
extern crate clap;
extern crate csv;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate safe_unwrap;
extern crate term_painter;
extern crate term_size;
extern crate textwrap;
extern crate try_from;

mod err;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::{App, Arg, SubCommand};
use err::{CheckError, ColumnConstraintsError, ColumnTypeError, ErrorLoc, ErrorAtLocation,
          HelpPrinter, Location, ResultLoc, SchemaLoadError, ValidationError, ValueError};
use std::{fmt, io, path, process};
use regex::Regex;
use safe_unwrap::SafeUnwrap;
use term_painter::{Attr, Color, ToStyle};
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
        r"^(:?[A-Z]+,?)*$"
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
        r"^([a-z][a-z0-9-]*)_(\d{4})(\d{2})(\d{2})_([a-z][a-z0-9-]*).csv$"
    ).expect("built-in Regex is broken. Please file a bug");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CsvxMetadata {
    pub table_name: String,
    pub date: NaiveDate,
    pub schema: String,
}

impl CsvxMetadata {
    fn is_schema(&self) -> bool {
        self.schema.starts_with("csvx-schema-")
    }
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

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ColumnType::String => write!(f, "STRING"),
            ColumnType::Bool => write!(f, "BOOL"),
            ColumnType::Integer => write!(f, "INTEGER"),
            ColumnType::Enum(ref variants) => write!(f, "ENUM({})", variants.join(",")),
            ColumnType::Decimal => write!(f, "DECIMAL"),
            ColumnType::Date => write!(f, "DATE"),
            ColumnType::DateTime => write!(f, "DATETIME"),
            ColumnType::Time => write!(f, "TIME"),
        }
    }
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

impl<S> TryFrom<S> for ColumnConstraints
    where S: AsRef<str>
{
    type Err = ColumnConstraintsError;

    fn try_from(s: S) -> Result<ColumnConstraints, Self::Err> {
        if !CONSTRAINT_RE.is_match(s.as_ref()) {
            return Err(ColumnConstraintsError::MalformedConstraints(s.as_ref().to_string()));
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
            _ => {
                if s.as_ref().starts_with("ENUM") {
                    return Err(ColumnTypeError::BadEnum(s.as_ref().to_owned()));
                }
                return Err(ColumnTypeError::UnknownType(s.as_ref().to_owned()));
            }

        }
    }
}

#[derive(Clone, Debug)]
pub struct CsvxColumnType {
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
                    _ => Err(ValueError::InvalidBool(s.as_ref().to_owned())),
                }
            }
            ColumnType::Integer => {
                // FIXME: check for leading zeros
                Ok(Some(Value::Integer(s.as_ref()
                                           .parse()
                                           .map_err(|_| {
                                                        ValueError::InvalidInt(s.as_ref()
                                                                                   .to_owned())
                                                    })?)))
            }
            ColumnType::Enum(ref variants) => {
                let v = s.as_ref().to_owned();
                if variants.contains(&v) {
                    Ok(Some(Value::Enum(v)))
                } else {
                    Err(ValueError::InvalidEnum(s.as_ref().to_owned(),
                        variants.clone()))
                }
            }
            ColumnType::Decimal => {
                if DECIMAL_RE.is_match(s.as_ref()) {
                    Ok(Some(Value::Decimal(s.as_ref().to_owned())))
                } else {
                    Err(ValueError::InvalidDecimal(s.as_ref().to_owned()))
                }
            }
            ColumnType::Date => {
                match DATE_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        Ok(Some(Value::Date(NaiveDate::from_ymd_opt(cap(c, 1),
                                                                    cap(c, 2),
                                                                    cap(c, 3))
              .ok_or_else(||{
                      ValueError::InvalidDate(s.as_ref().to_owned ()) }
                      )?)))
                    }
                    None => Err(ValueError::InvalidDate(s.as_ref().to_owned())),
                }
            }
            ColumnType::DateTime => {
                match DATETIME_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        let dt =
                            NaiveDate::from_ymd_opt(cap(c, 1), cap(c, 2), cap(c, 3))
                                .ok_or_else(|| ValueError::InvalidDate(s.as_ref().to_string()))?;
                        Ok(Some(Value::DateTime(dt.and_hms_opt(cap(c, 4), cap(c, 5), cap(c, 6))
                                                    .ok_or_else(|| {
                                                                    ValueError::InvalidTime (s.as_ref ().to_string ()) })?)))
                    }
                    None => Err(ValueError::InvalidDateTime(s.as_ref
                        ().to_string())),
                }
            }
            ColumnType::Time => {
                match TIME_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        Ok(Some(Value::Time(NaiveTime::from_hms_opt(cap(c, 1),
                                                                    cap(c, 2),
                                                                    cap(c, 3))
                                                    .ok_or_else(||
                                                        ValueError::InvalidTime (s.as_ref ().to_string ()))?)))
                    }
                    None => Err(ValueError::InvalidTime(s.as_ref().to_string())),
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct CsvxSchema {
    columns: Vec<CsvxColumnType>,
}

impl CsvxSchema {
    fn from_file<P: AsRef<path::Path>>
        (filename: P)
         -> Result<CsvxSchema, ErrorAtLocation<SchemaLoadError, Location>> {

        // have a copy of the filename as a string ready for error locations
        let filename_s = filename.as_ref().to_string_lossy().to_string();

        let mut rdr = csv::Reader::from_file(filename)
            .err_at(|| Location::File(filename_s.clone()))?
            .has_headers(false);

        let mut it = rdr.decode();
        let header: Option<Result<(String, String, String, String), _>> = it.next();

        let mut columns = Vec::new();

        match header {
            None => {
                return Err(SchemaLoadError::MissingHeader.at(Location::FileLine(filename_s, 1)))
            }
            Some(res) => {
                let fields = res.err_at(|| Location::File(filename_s.clone()))?;
                if fields.0 != "id" || fields.1 != "type" || fields.2 != "constraints" ||
                   fields.3 != "description" {

                    return Err(SchemaLoadError::BadHeader.at(Location::FileLine(filename_s, 1)));
                }

                for (recno, rec) in it.enumerate() {
                    let (id, ty, constraints, desc) =
                        rec.err_at(|| Location::FileLine(filename_s.clone(), 1))?;
                    let lineno = recno + 2;

                    // check identifier
                    if !IDENT_UNDERSCORE_RE.is_match(&id.as_str()) {
                        return Err(SchemaLoadError::BadIdentifier(id)
                                       .at(Location::FileLineField(filename_s, lineno, 1)));
                    }

                    // create type
                    let col_type = match ColumnType::try_from(ty.as_str()) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(SchemaLoadError::BadType(e)
                                           .at(Location::FileLineField(filename_s, lineno, 1)))
                        }
                    };

                    // create constraints
                    let col_constraints = match ColumnConstraints::try_from(constraints.as_str()) {
                        Ok(v) => v,
                        // FIXME: location
                        Err(e) => {
                            return Err(SchemaLoadError::BadConstraints(e)
                                           .at(Location::FileLine(filename_s, lineno)))
                        }
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

    fn validate_file<P: AsRef<path::Path>>
        (&self,
         filename: P)
         -> Result<(), Vec<ErrorAtLocation<ValidationError, Location>>> {
        let filename_s = filename.as_ref().to_string_lossy().to_string();

        let mut rdr = csv::Reader::from_file(filename)
            .map_err(|e| vec![e.at(Location::File(filename_s.clone()))])?
            .has_headers(true);

        let headers = rdr.headers()
            .map_err(|e| vec![e.at(Location::FileLine(filename_s.clone(), 1))])?;

        if headers.len() != self.columns.len() {
            return Err(vec![ValidationError::MissingHeaders
                                .at(Location::FileLine(filename_s.clone(), 1))]);
        }

        let mut errs = Vec::new();

        for (idx, (spec, actual)) in self.columns.iter().zip(headers.iter()).enumerate() {
            if spec.id.as_str() != actual {
                errs.push(ValidationError::HeaderMismatch(actual.to_string())
                              .at(Location::FileLineField(filename_s.clone(), 1, idx + 1)));
            }
        }

        // bail if headers are incorrect
        if errs.len() != 0 {
            return Err(errs);
        }

        for (rowid, row) in rdr.records().enumerate() {
            let lineno = rowid + 2;

            // bail early if we cannot read the fields, this is probably a
            // major csv issue
            let fields = row.map_err(|e| vec![e.at(Location::FileLine(filename_s.clone(), 1))])?;

            for (idx, (col, value)) in self.columns.iter().zip(fields.iter()).enumerate() {
                if let Err(e) = col.validate_value(value) {
                    let col_idx = idx + 1;

                    errs.push(ValidationError::ValueError(e)
                                  .at(Location::FileLineField(filename_s.clone(),
                                                              lineno,
                                                              col_idx)));
                    continue;
                }
            }
        }

        if errs.len() != 0 {
            return Err(errs);
        } else {
            Ok(())
        }

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

            Some(CsvxMetadata {
                     table_name: table_name,
                     date: match NaiveDate::from_ymd_opt(year, month, day) {
                         Some(d) => d,
                         None => return None,
                     },
                     schema: schema,
                 })
        }
        None => None,
    }
}

/// Check input files against schema.
///
/// Fatal and schema errors are returned as errors; failing input files just
/// result in a return value of `Ok(false)`.
fn cmd_check<P: AsRef<path::Path>, Q: AsRef<path::Path>>
    (schema_path: P,
     input_files: Vec<Q>)
     -> Result<bool, ErrorAtLocation<CheckError, Location>> {

    // ensure schema_path evaluates to a real utf8 path
    let schema_path_s = schema_path
        .as_ref()
        .to_str()
        .ok_or_else(|| CheckError::SchemaPathUtf8Error.at(Location::Unspecified))?
        .to_owned();

    // get filename portion
    let meta_fn = schema_path
        .as_ref()
        .to_owned()
        .file_name()
        .ok_or_else(|| CheckError::SchemaNotAFile.at(Location::File(schema_path_s.clone())))?
        .to_str()
        .safe_unwrap("already verified UTF8")
        .to_owned();

    let meta =
        parse_filename(meta_fn.clone())
            .ok_or_else(|| {
                            CheckError::InvalidCsvxFilename(meta_fn).at(Location::File (schema_path_s.clone()))
                        })?;

    if !meta.is_schema() {
        return Err(CheckError::NotASchema.at(Location::File(schema_path_s.clone())));
    }

    // load schema
    let schema = CsvxSchema::from_file(schema_path)
        .map_err(|e| e.convert())?;

    // schema validated correctly, reward user with a checkmark
    println!("{} {}",
             Color::Green.paint(Attr::Bold.paint("✓")),
             Attr::Bold.paint(schema_path_s));

    let mut all_good = true;
    for input_file in input_files {
        match schema.validate_file(&input_file) {
            Ok(()) => println!("{} {}",
                 Color::Green.paint(Attr::Bold.paint("✓")),
                 input_file.as_ref().to_string_lossy()),
            Err(errs) => {
                all_good = false;
                println!("{} {}",
                         Color::Red.paint(Attr::Bold.paint("✗")),
                         input_file.as_ref().to_string_lossy());
                for e in errs {
                    e.print_help();
                }
            }
        }
    }

    Ok(all_good)
}

fn main() {
    let app = App::new("csvx")
        .version("5.1.0")
        .about("csvx utility")
        .subcommand(SubCommand::with_name("check")
                        .about("Check csvx files for conformance")
                        .arg(Arg::with_name("schema_path")
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
            let res = cmd_check(cmd.matches
                                    .value_of("schema_path")
                                    .safe_unwrap("required argument"),
                                cmd.matches
                                    .values_of("input_files")
                                    .map(|v| v.collect())
                                    .unwrap_or_else(|| Vec::new()));

            match res {
                Err(e) => {
                    // display fatal error:
                    e.print_help();
                    process::exit(1);
                }
                Ok(result) => {
                    // the errors have already been displayed by `cmd_check()`
                    // we use exit status `2` for validation but non-fatal
                    // errors
                    process::exit(if result { 0 } else { 2 });
                }
            }
        }
        _ => app.write_help(&mut io::stdout()).unwrap(),
    };

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
        assert_eq!(parse_filename("zoo-nyc_20170401_animals-2.csv").unwrap(),
                   CsvxMetadata {
                       table_name: "zoo-nyc".to_owned(),
                       date: NaiveDate::from_ymd(2017, 04, 01),
                       schema: "animals-2".to_owned(),
                   });
    }

}
