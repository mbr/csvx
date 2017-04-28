use csv;

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
