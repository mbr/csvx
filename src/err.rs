use csv;
use std::{cmp, error, fmt};
use std::error::Error;
use term_painter::{Attr, Color, ToStyle};
use term_size;
use textwrap;

/// Types with Long-help available
pub trait Helpful {
    /// Return a long help message about the error
    fn help(&self) -> String;
}

/// A location in input data
#[derive(Clone, Debug)]
pub enum Location {
    // /// File, Line, Colum
    // ///
    // /// Note that Column refers to character columns
    // FileLineColumn(String, usize, usize),
    /// File, Row, Field
    ///
    /// Fields are CSV columns (compare `FileLineColumn`)
    FileLineField(String, usize, usize),

    /// File, Line
    FileLine(String, usize),

    /// File
    File(String),

    /// Unspecified location
    Unspecified,
}

impl Default for Location {
    fn default() -> Location {
        Location::Unspecified
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            // Location::FileLineColumn(ref file, line, col) =>
            // write!(f, "{}:{}:{}", file, line, col),
            Location::FileLineField(ref file, row, field) => {
                write!(f, "{}:{}[field {}]", file, row, field)
            }
            Location::FileLine(ref file, line) => write!(f, "{}:{}]", file, line),
            Location::File(ref file) => write!(f, "{}", file),
            Location::Unspecified => Ok(()),
        }
    }
}

#[derive(Debug)]
pub struct ErrorAtLocation<E, L> {
    location: L,
    error: E,
}

pub trait ErrorLoc<E, L>: Sized {
    #[inline]
    fn at(self, location: L) -> ErrorAtLocation<E, L>;
}

impl<E: error::Error, F: Into<E>, L: Default> ErrorLoc<E, L> for F {
    #[inline]
    fn at(self, location: L) -> ErrorAtLocation<E, L> {
        ErrorAtLocation::new(location, self.into())
    }
}

pub trait ResultLoc<V, E: ErrorLoc<E, L>, L>: Sized {
    #[inline]
    fn error_at(self, location: L) -> Result<V, ErrorAtLocation<E, L>>;

    #[inline]
    fn err_at<F: FnOnce() -> L>(self, floc: F) -> Result<V, ErrorAtLocation<E, L>> {
        self.error_at(floc())
    }
}

impl<V, E: error::Error, F: Into<E>, L: Default> ResultLoc<V, E, L> for Result<V, F> {
    #[inline]
    fn error_at(self, location: L) -> Result<V, ErrorAtLocation<E, L>> {
        self.map_err(|f| f.at(location))
    }
}

impl<E, L: Default> ErrorAtLocation<E, L> {
    pub fn new<F: Into<E>>(location: L, error: F) -> ErrorAtLocation<E, L> {
        ErrorAtLocation {
            location: location,
            error: error.into(),
        }
    }

    pub fn from_error<F: Into<E>>(other: F) -> ErrorAtLocation<E, L> {
        ErrorAtLocation::new(L::default(), other.into())
    }

    pub fn convert<F: From<E>>(self) -> ErrorAtLocation<F, L> {
        ErrorAtLocation {
            location: self.location,
            error: self.error.into(),
        }
    }
}

/// Supports printing out help
pub trait HelpPrinter {
    fn print_help(&self);
}

impl<E: fmt::Display + Helpful> HelpPrinter for ErrorAtLocation<E, Location> {
    fn print_help(&self) {
        println!("{}{} {}",
                 Attr::Bold.paint((Color::Red.paint("error"))),
                 Attr::Bold.paint(":"),
                 Attr::Bold.paint(self.error()));
        match *self.location() {
            Location::Unspecified => (),
            _ => println!("  --> {}", Color::Yellow.paint(self.location())),
        }

        let dims = term_size::dimensions().unwrap_or((80, 25));

        let term_width = cmp::max(dims.0, 4);
        let out = textwrap::wrap(self.error.help().as_str(), term_width - 3)
            .into_iter()
            .map(|line| textwrap::indent(line.as_str(), "   "))
            .fold(String::new(), |s1, s2| s1 + s2.as_str());
        println!("{}", out);
    }
}

impl<E, L> ErrorAtLocation<E, L> {
    pub fn error(&self) -> &E {
        &self.error
    }

    // pub fn into_error(self) -> E {
    //     self.error
    // }

    pub fn location(&self) -> &L {
        &self.location
    }
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for ErrorAtLocation<E, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.location() {
            _ => write!(f, "{}: {}", self.location(), self.error()),
        }
    }
}

impl<E: error::Error, L: fmt::Debug + fmt::Display> error::Error for ErrorAtLocation<E, L> {
    fn description(&self) -> &str {
        self.error.description()
    }

    fn cause(&self) -> Option<&error::Error> {
        Some(self.error())
    }
}

impl<E, L: Default> From<E> for ErrorAtLocation<E, L> {
    fn from(e: E) -> ErrorAtLocation<E, L> {
        ErrorAtLocation::from_error(e)
    }
}

/// Top-level check error
///
/// Encapsulates all fatal errors that can occur when checking files against
/// a schema
#[derive(Debug)]
pub enum CheckError {
    /// Filename does not indicate schema
    NotASchema,

    /// Cannot open because not a file
    SchemaNotAFile,

    /// Filename invalid according to CSVX spec
    InvalidCsvxFilename,

    /// Error loading schema
    SchemaLoadError(SchemaLoadError),

    /// Path is not valid UTF8
    SchemaPathUtf8Error,
}

impl From<SchemaLoadError> for CheckError {
    fn from(e: SchemaLoadError) -> CheckError {
        CheckError::SchemaLoadError(e)
    }
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            write!(f, "{}", cause)
        } else {
            write!(f, "{}", self.description())
        }
    }
}

impl error::Error for CheckError {
    fn description(&self) -> &str {
        match *self {
            CheckError::NotASchema => "not a schema",
            CheckError::SchemaNotAFile => "schema is not a file",
            CheckError::InvalidCsvxFilename => "filename is not a valid CSVX filename",
            CheckError::SchemaLoadError(_) => "could not load schema",
            CheckError::SchemaPathUtf8Error => "filename UTF8 decoding error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            CheckError::SchemaLoadError(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Helpful for CheckError {
    fn help(&self) -> String {
        match *self {
            CheckError::NotASchema => {
                "The file you provided is not a schema. The third field of \
                the filename must be of the form `csvx-schema-` followed by \
                the version number. As an example, when defining a schema \
                named `animals-2`, with a date of Dec 31st, 2015 and CSVX \
                version 5, the resulting filename should be: \
                `animals-2_20151231_csvx-schema-5.csv`. \n\
                Note that filenames are case sensitive!"
                        .to_owned()
            }
            CheckError::SchemaNotAFile => "The schema you supplied is not a valid file".to_owned(),
            CheckError::InvalidCsvxFilename => {
                "The filename provided is not in a valid CSVX form. CSVX \
                filenames have three components: The table name, date and \
                schema name. All components must be lowercase letters, \
                numbers or hyphens and start with a letter.\n\n\
                The table name component identifies the file or rather the \
                table it was exported from.\n\
                The date component is for the date the file was exported on.\n\
                The schema name component indicates which schema should be \
                used to validate its contents.\n\n\
                Example: With a table name of `nyc-zoo`, a date of Dec 31st, \
                2015 and using a schema named `animals-2`, the resulting \
                filename should be `nyc-zoo_20151231_animals-2.csv`."
                        .to_owned()
            }
            CheckError::SchemaPathUtf8Error => {
                "The filename you supplied contained UTF-8 errors. CSVX \
                filenames should only contain ASCII characters, please rename \
                the file and try again."
                        .to_owned()
            }
            CheckError::SchemaLoadError(ref e) => e.help(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ColumnConstraintsError {
    MalformedConstraints(String),
    UnknownConstraint(String),
}

impl fmt::Display for ColumnConstraintsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ColumnConstraintsError::MalformedConstraints(ref s) => {
                write!(f, "malformed constraints: `{}`", s)
            }
            ColumnConstraintsError::UnknownConstraint(ref s) => {
                write!(f, "unknown constraint: `{}`", s)
            }
        }
    }
}

impl error::Error for ColumnConstraintsError {
    fn description(&self) -> &str {
        match *self {
            ColumnConstraintsError::MalformedConstraints(_) => "malformed constraints",
            ColumnConstraintsError::UnknownConstraint(_) => "unknown constraint",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            _ => None,
        }
    }
}

impl Helpful for ColumnConstraintsError {
    fn help(&self) -> String {
        match *self {
            ColumnConstraintsError::MalformedConstraints(_) => {
                "The constraints could be not recognized. Constraints must be \
                all uppercase letters, comma-separated, with no spaces in \
                between."
                        .to_owned()
            }
            ColumnConstraintsError::UnknownConstraint(_) => {
                "The constraint is not known to be a valid constraint. Valid \
                constraints are `NULLABLE` and `UNIQUE`."
                        .to_owned()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ColumnTypeError {
    /// Unknown column type
    UnknownType(String),

    /// Type is intended to be an `ENUM`, but invalid
    BadEnum(String),
}

impl fmt::Display for ColumnTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ColumnTypeError::UnknownType(ref s) => write!(f, "unknown column type `{}`", s),
            ColumnTypeError::BadEnum(ref s) => write!(f, "bad enum `{}`", s),
        }
    }
}

impl error::Error for ColumnTypeError {
    fn description(&self) -> &str {
        match *self {
            ColumnTypeError::UnknownType(_) => "unknown column type",
            ColumnTypeError::BadEnum(_) => "bad enum",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl Helpful for ColumnTypeError {
    fn help(&self) -> String {
        match *self {
            ColumnTypeError::UnknownType(_) => {
                "The column type specified is not known. Valid types are \
                `STRING`, `BOOL`, `INTEGER`, `ENUM(...)`, `DECIMAL`, \
                `DATE`, `DATETIME` and `TIME`"
                        .to_owned()
            }
            ColumnTypeError::BadEnum(_) => {
                "The `ENUM` specified is not valid. Enums must be of the form \
                `ENUM(V1,V2,V3,` ... `)`. Note that variants must be of \
                uppercase letters and numbers only, separated by commas, \
                with no spaces allowed in between"
                        .to_owned()
            }
        }
    }
}

/// Schema loading error
#[derive(Debug)]
pub enum SchemaLoadError {
    /// Generic CSV parsing error
    Csv(csv::Error),

    /// No header present (because file is empty)
    MissingHeader,

    /// Header columns incorrect
    BadHeader,

    /// The identifier is invalid
    BadIdentifier(String),

    /// Bad column type
    BadType(ColumnTypeError),

    /// Bad constraints
    BadConstraints(ColumnConstraintsError),
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SchemaLoadError::BadIdentifier(ref ident) => write!(f, "bad identifier `{}`", ident),
            _ => {
                if let Some(cause) = self.cause() {
                    write!(f, "{}", cause)
                } else {
                    write!(f, "{}", self.description())
                }
            }
        }

    }
}

impl error::Error for SchemaLoadError {
    fn description(&self) -> &str {
        match *self {
            SchemaLoadError::Csv(_) => "invalid CSV",
            SchemaLoadError::MissingHeader => "missing header",
            SchemaLoadError::BadHeader => "header is invalid",
            SchemaLoadError::BadIdentifier(_) => "bad identifier",
            SchemaLoadError::BadType(_) => "bad type",
            SchemaLoadError::BadConstraints(_) => "invalid constraints",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            SchemaLoadError::Csv(ref e) => Some(e),
            SchemaLoadError::BadType(ref e) => Some(e),
            SchemaLoadError::BadConstraints(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Helpful for SchemaLoadError {
    fn help(&self) -> String {
        match *self {
            SchemaLoadError::Csv(_) => {
                "The CSV file could not be loaded. Please ensure that the \
                file exists and is a valid CSV file according to the \
                CSVX specification as well as RFC4180.\n\n\
                The most common errors in CSV files are wrong field \
                separators (only commas are valid separators) or invalid \
                decimal separators (decimal point must be dots `.`, not \
                commas or other locale specific characters."
                        .to_owned()
            }
            SchemaLoadError::MissingHeader => "The CSV file has no header; it's empty.".to_owned(),
            SchemaLoadError::BadHeader => {
                "The CSV file has an invalid header. A valid header for \
                a schema file contains exactly four fields and looks like \
                this: \n\n\
                id,type,constraints,description"
                        .to_owned()
            }
            SchemaLoadError::BadIdentifier(_) => {
                "A valid identifier must start with a lowercase letter and \
                contain only lowercase letters, numbers or underscores."
                        .to_owned()
            }
            SchemaLoadError::BadType(ref e) => e.help(),
            SchemaLoadError::BadConstraints(ref e) => e.help(),
        }
    }
}

impl From<csv::Error> for SchemaLoadError {
    fn from(e: csv::Error) -> SchemaLoadError {
        SchemaLoadError::Csv(e)
    }
}

#[derive(Debug)]
pub enum ValidationError {
    /// Generic CSV error
    Csv(csv::Error),

    /// No headers found, file is empty
    MissingHeaders,

    /// Header in file does not match specification
    HeaderMismatch(String),

    /// A row has a different length then all the others
    RowLengthMismatch,

    /// A value error occured
    ValueError(super::CsvxColumnType, ValueError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ValidationError::ValueError(ref coltype, ref e) => {
                write!(f,
                       "could not parse field `{}` as `{}`: {}",
                       coltype.id,
                       coltype.ty,
                       e)
            }
            _ => {
                if let Some(cause) = self.cause() {
                    write!(f, "{}", cause)
                } else {
                    write!(f, "{}", self.description())
                }
            }
        }

    }
}

impl error::Error for ValidationError {
    fn description(&self) -> &str {
        match *self {
            ValidationError::Csv(_) => "invalid CSV",
            ValidationError::MissingHeaders => "missing headers",
            ValidationError::HeaderMismatch(_) => "header mismatch",
            ValidationError::RowLengthMismatch => "row length mismatch",
            ValidationError::ValueError(_, _) => "value error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            ValidationError::Csv(ref e) => Some(e),
            ValidationError::ValueError(_, ref e) => Some(e),
            _ => None,
        }
    }
}

impl Helpful for ValidationError {
    fn help(&self) -> String {
        "FIXME".to_owned()
    }
}

impl From<csv::Error> for ValidationError {
    fn from(e: csv::Error) -> ValidationError {
        ValidationError::Csv(e)
    }
}

#[derive(Debug)]
pub enum ValueError {
    /// A field that was not NULLABLE had no value
    NonNullable,

    /// Invalid boolean value
    InvalidBool,

    /// Invalid integer
    InvalidInt,

    /// Invalid enum value
    InvalidEnum,

    /// Invalid decimal value
    InvalidDecimal,

    /// Invalid date value
    InvalidDate,

    /// Invalid datetime value
    InvalidDateTime,

    /// Invalid time value
    InvalidTime,

    // FIXME: Add OutOfRange and other errors
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl error::Error for ValueError {
    fn description(&self) -> &str {
        match *self {
            ValueError::NonNullable => "field is not nullable",
            ValueError::InvalidBool => "invalid boolean",
            ValueError::InvalidInt => "invalid integer",
            ValueError::InvalidEnum => "invalid enum",
            ValueError::InvalidDecimal => "invalid decimal",
            ValueError::InvalidDate => "invalid date",
            ValueError::InvalidDateTime => "invalid datetime",
            ValueError::InvalidTime => "invalid time",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
