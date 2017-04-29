use csv;
use std::{cmp, error, fmt};
use std::error::Error;
use term_painter::{Attr, Color, ToStyle};
use term_size;
use textwrap;

pub trait Helpful {
    fn help(&self) -> String;
}

#[derive(Clone, Debug)]
pub enum Location {
    FileLineColumn(String, usize, usize),
    FileRowField(String, usize, usize),
    FileLine(String, usize),
    File(String),
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
            Location::FileLineColumn(ref file, line, col) => write!(f, "{}:{}:{}", file, line, col),
            Location::FileRowField(ref file, row, field) => {
                write!(f, "{}:{}[{}]", file, row, field)
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

    pub fn into_error(self) -> E {
        self.error
    }

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

#[derive(Debug)]
pub enum CheckError {
    NotASchema,
    SchemaNotAFile,
    InvalidCsvxFilename,
    SchemaLoadError(SchemaLoadError),
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
    MalformedConstraint,
    UnknownConstraint(String),
}

#[derive(Clone, Debug)]
pub enum ColumnTypeError {
    UnknownType,
}

#[derive(Debug)]
pub enum SchemaLoadError {
    Csv(csv::Error),
    MissingHeader,
    BadHeader,
    BadIdentifier(usize, String),
    BadType(usize, ColumnTypeError),
    BadConstraints(usize, ColumnConstraintsError),
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            write!(f, "{}", cause)
        } else {
            write!(f, "{}", self.description())
        }
    }
}

impl error::Error for SchemaLoadError {
    fn description(&self) -> &str {
        match *self {
            _ => "TBW",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            _ => None,
        }
    }
}

impl Helpful for SchemaLoadError {
    fn help(&self) -> String {
        "FIXME".to_owned()
    }
}

impl From<csv::Error> for SchemaLoadError {
    fn from(e: csv::Error) -> SchemaLoadError {
        SchemaLoadError::Csv(e)
    }
}

#[derive(Debug)]
pub enum ValidationError {
    Csv(csv::Error),
    MissingHeaders,
    HeaderMismatch(usize, String),
    RowLengthMismatch(usize),
    ValueError(usize, usize, ValueError),
}

#[derive(Debug)]
pub enum ValueError {
    NonNullable,
    InvalidBool,
    InvalidInt,
    InvalidEnum,
    InvalidDecimal,
    InvalidDate,
    InvalidDateTime,
    InvalidTime,
}
