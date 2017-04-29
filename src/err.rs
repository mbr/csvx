use csv;
use std::{cmp, error, fmt};
use std::error::Error;
use term_painter::{Attr, Color, ToStyle};
use term_size;
use textwrap;

#[derive(Clone, Debug)]
pub enum Location {
    FileLineColumn(String, usize, usize),
    FileRowField(String, usize, usize),
    FileLine(String, usize),
    File(String),
    Unspecified,
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
pub struct ErrorWithLocation<E> {
    location: Location,
    error: E,
}

impl<E> ErrorWithLocation<E> {
    pub fn new<F: Into<E>>(location: Location, error: F) -> ErrorWithLocation<E> {
        ErrorWithLocation {
            location: location,
            error: error.into(),
        }
    }

    pub fn from_error<F: Into<E>>(other: F) -> ErrorWithLocation<E> {
        ErrorWithLocation::new(Location::Unspecified, other.into())
    }
}

impl<E: fmt::Display> ErrorWithLocation<E> {
    pub fn print_help(&self) {
        println!("{}{} {}",
                 Attr::Bold.paint((Color::Red.paint("error"))),
                 Attr::Bold.paint(":"),
                 Attr::Bold.paint(self.error()));
        match *self.location() {
            Location::Unspecified => (),
            _ => println!("  --> {}", Color::Yellow.paint(self.location())),
        }

        let dims = term_size::dimensions().unwrap_or((80, 25));
        let msg = "TBW";

        let term_width = cmp::max(dims.0, 4);
        let out = textwrap::wrap(msg, term_width - 3)
            .into_iter()
            .map(|line| textwrap::indent(line.as_str(), "   "))
            .fold(String::new(), |s1, s2| s1 + s2.as_str());
        println!("{}", out);
    }
}

impl<E> ErrorWithLocation<E> {
    pub fn error(&self) -> &E {
        &self.error
    }

    pub fn into_error(self) -> E {
        self.error
    }

    pub fn location(&self) -> &Location {
        &self.location
    }
}

impl<E: fmt::Display> fmt::Display for ErrorWithLocation<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.location() {
            Location::Unspecified => write!(f, "{}", self.error()),
            _ => write!(f, "{}: {}", self.location(), self.error()),
        }
    }
}

impl<E: error::Error> error::Error for ErrorWithLocation<E> {
    fn description(&self) -> &str {
        self.error.description()
    }

    fn cause(&self) -> Option<&error::Error> {
        Some(self.error())
    }
}

impl<E> From<E> for ErrorWithLocation<E> {
    fn from(e: E) -> ErrorWithLocation<E> {
        ErrorWithLocation::from_error(e)
    }
}

#[derive(Debug)]
pub enum CheckError {
    NotASchema,
    SchemaNotAFile,
    InvalidCsvxFilename,
    SchemaLoadError(SchemaLoadError),
    SchemaPathInvalid,
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
            _ => "FIXME",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            _ => None,
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
