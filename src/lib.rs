extern crate chrono;
extern crate csv;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate safe_unwrap;
extern crate term_painter;
extern crate term_size;
extern crate textwrap;
extern crate try_from;

pub mod err;
mod regexes;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use err::{ColumnConstraintsError, ColumnTypeError, ErrorLoc, ErrorAtLocation, Location, ResultLoc,
          SchemaLoadError, ValidationError, ValueError};
use std::{fmt, fs, path, slice};
use std::io::Read;
use safe_unwrap::SafeUnwrap;
use regexes::{IDENT_UNDERSCORE_RE, ENUM_EXPR_RE, CONSTRAINT_RE, DECIMAL_RE, DATE_RE, DATETIME_RE,
              FN_RE, TIME_RE};
use try_from::TryFrom;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvxMetadata {
    pub table_name: String,
    pub date: NaiveDate,
    pub schema: String,
}

impl CsvxMetadata {
    pub fn is_schema(&self) -> bool {
        self.schema.starts_with("csvx-schema-")
    }
}

#[derive(Clone, Debug)]
pub enum ColumnType {
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
pub struct ColumnConstraints {
    pub nullable: bool,
    pub unique: bool,
}

impl Default for ColumnConstraints {
    fn default() -> ColumnConstraints {
        ColumnConstraints {
            nullable: false,
            unique: false,
        }
    }
}

impl fmt::Display for ColumnConstraints {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = Vec::new();
        if self.nullable {
            parts.push("NULLABLE");
        }
        if self.unique {
            parts.push("UNIQUE");
        }
        write!(f, "{}", parts.join(","))
    }
}

impl<S> TryFrom<S> for ColumnConstraints
where
    S: AsRef<str>,
{
    type Err = ColumnConstraintsError;

    fn try_from(s: S) -> Result<ColumnConstraints, Self::Err> {
        if !CONSTRAINT_RE.is_match(s.as_ref()) {
            return Err(ColumnConstraintsError::MalformedConstraints(
                s.as_ref().to_string(),
            ));
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
                _ => {
                    return Err(ColumnConstraintsError::UnknownConstraint(
                        s.as_ref().to_string(),
                    ))
                }
            }

        }

        Ok(ccs)
    }
}

impl<S> TryFrom<S> for ColumnType
where
    S: AsRef<str>,
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
    pub id: String,
    pub ty: ColumnType,
    pub constraints: ColumnConstraints,
    pub description: String,
}

#[derive(Clone, Debug)]
pub enum Value {
    String(String),
    Bool(bool),
    Integer(i64),
    Enum(usize),
    Decimal(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Time(NaiveTime),
}

impl Value {
    pub fn to_string(self) -> Option<String> {
        match self {
            Value::String(s) => Some(s),
            Value::Decimal(d) => Some(d),
            _ => None,
        }
    }

    pub fn to_bool(self) -> Option<bool> {
        if let Value::Bool(val) = self {
            Some(val)
        } else {
            None
        }
    }

    pub fn to_i64(self) -> Option<i64> {
        if let Value::Integer(val) = self {
            Some(val)
        } else {
            None
        }
    }

    pub fn to_date(self) -> Option<NaiveDate> {
        if let Value::Date(val) = self {
            Some(val)
        } else {
            None
        }
    }

    pub fn to_datetime(self) -> Option<NaiveDateTime> {
        if let Value::DateTime(val) = self {
            Some(val)
        } else {
            None
        }
    }

    pub fn to_time(self) -> Option<NaiveTime> {
        if let Value::Time(val) = self {
            Some(val)
        } else {
            None
        }
    }

    pub fn to_usize(self) -> Option<usize> {
        if let Value::Enum(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl CsvxColumnType {
    pub fn validate_value<S: AsRef<str>>(&self, s: &S) -> Result<Option<Value>, ValueError> {
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
                Ok(Some(Value::Integer(s.as_ref().parse().map_err(|_| {
                    ValueError::InvalidInt(s.as_ref().to_owned())
                })?)))
            }
            ColumnType::Enum(ref variants) => {
                let v = s.as_ref();

                if let Some(p) = variants.iter().position(|e| e == v) {
                    Ok(Some(Value::Enum(p)))
                } else {
                    Err(ValueError::InvalidEnum(
                        s.as_ref().to_owned(),
                        variants.clone(),
                    ))
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
                        Ok(Some(Value::Date(
                            NaiveDate::from_ymd_opt(cap(c, 1), cap(c, 2), cap(c, 3))
                                .ok_or_else(|| ValueError::InvalidDate(s.as_ref().to_owned()))?,
                        )))
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
                        Ok(Some(Value::DateTime(
                            dt.and_hms_opt(cap(c, 4), cap(c, 5), cap(c, 6)).ok_or_else(
                                || {
                                    ValueError::InvalidTime(s.as_ref().to_string())
                                },
                            )?,
                        )))
                    }
                    None => Err(ValueError::InvalidDateTime(s.as_ref().to_string())),
                }
            }
            ColumnType::Time => {
                match TIME_RE.captures(s.as_ref()) {
                    Some(ref c) => {
                        Ok(Some(Value::Time(
                            NaiveTime::from_hms_opt(cap(c, 1), cap(c, 2), cap(c, 3))
                                .ok_or_else(|| ValueError::InvalidTime(s.as_ref().to_string()))?,
                        )))
                    }
                    None => Err(ValueError::InvalidTime(s.as_ref().to_string())),
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CsvxSchema {
    columns: Vec<CsvxColumnType>,
}

impl CsvxSchema {
    pub fn iter_columns(&self) -> slice::Iter<CsvxColumnType> {
        self.columns.iter()
    }

    pub fn col_idx(&self, col: &str) -> Option<usize> {
        self.columns.iter().position(|c| col == c.id)
    }

    pub fn from_file<P: AsRef<path::Path>>(
        filename: P,
    ) -> Result<CsvxSchema, ErrorAtLocation<SchemaLoadError, Location>> {

        // have a copy of the filename as a string ready for error locations
        let filename_s: String = filename.as_ref().to_string_lossy().into_owned();
        let mut file = fs::File::open(filename).err_at(|| {
            Location::File(filename_s.clone())
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).err_at(|| {
            Location::File(filename_s.clone())
        })?;

        Self::from_string(contents.as_str(), filename_s.as_ref())
    }

    pub fn from_string(
        src: &str,
        filename: &str,
    ) -> Result<CsvxSchema, ErrorAtLocation<SchemaLoadError, Location>> {
        // have a copy of the filename as a string ready for error locations
        let filename_s = filename.to_string();

        let mut rdr = csv::Reader::from_string(src).has_headers(false);

        let mut it = rdr.decode();
        let header: Option<Result<(String, String, String, String), _>> = it.next();

        let mut columns = Vec::new();

        match header {
            None => {
                return Err(SchemaLoadError::MissingHeader.at(Location::FileLine(
                    filename_s,
                    1,
                )))
            }
            Some(res) => {
                let fields = res.err_at(|| Location::File(filename_s.clone()))?;
                if fields.0 != "id" || fields.1 != "type" || fields.2 != "constraints" ||
                    fields.3 != "description"
                {

                    return Err(SchemaLoadError::BadHeader.at(
                        Location::FileLine(filename_s, 1),
                    ));
                }

                for (recno, rec) in it.enumerate() {
                    let (id, ty, constraints, desc) =
                        rec.err_at(|| Location::FileLine(filename_s.clone(), 1))?;
                    let lineno = recno + 2;

                    // check identifier
                    if !IDENT_UNDERSCORE_RE.is_match(&id.as_str()) {
                        return Err(SchemaLoadError::BadIdentifier(id).at(
                            Location::FileLineField(
                                filename_s,
                                lineno,
                                1,
                            ),
                        ));
                    }

                    // create type
                    let col_type = match ColumnType::try_from(ty.as_str()) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(SchemaLoadError::BadType(e).at(Location::FileLineField(
                                filename_s,
                                lineno,
                                1,
                            )))
                        }
                    };

                    // create constraints
                    let col_constraints = match ColumnConstraints::try_from(constraints.as_str()) {
                        Ok(v) => v,
                        // FIXME: location
                        Err(e) => {
                            return Err(SchemaLoadError::BadConstraints(e).at(Location::FileLine(
                                filename_s,
                                lineno,
                            )))
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

    pub fn validate_file<P: AsRef<path::Path>>(
        &self,
        filename: P,
    ) -> Result<(), Vec<ErrorAtLocation<ValidationError, Location>>> {
        let filename_s = filename.as_ref().to_string_lossy().to_string();

        let mut rdr = csv::Reader::from_file(filename)
            .map_err(|e| vec![e.at(Location::File(filename_s.clone()))])?
            .has_headers(true);

        let headers = rdr.headers().map_err(|e| {
            vec![e.at(Location::FileLine(filename_s.clone(), 1))]
        })?;

        if headers.len() != self.columns.len() {
            return Err(vec![
                ValidationError::MissingHeaders.at(Location::FileLine(
                    filename_s.clone(),
                    1,
                )),
            ]);
        }

        let mut errs = Vec::new();

        for (idx, (spec, actual)) in self.columns.iter().zip(headers.iter()).enumerate() {
            if spec.id.as_str() != actual {
                errs.push(ValidationError::HeaderMismatch(actual.to_string()).at(
                    Location::FileLineField(filename_s.clone(), 1, idx + 1),
                ));
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
            let fields = row.map_err(
                |e| vec![e.at(Location::FileLine(filename_s.clone(), 1))],
            )?;

            for (idx, (col, value)) in self.columns.iter().zip(fields.iter()).enumerate() {
                if let Err(e) = col.validate_value(value) {
                    let col_idx = idx + 1;

                    errs.push(ValidationError::ValueError(e).at(Location::FileLineField(
                        filename_s.clone(),
                        lineno,
                        col_idx,
                    )));
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

    pub fn parse_row<T: AsRef<[String]>>(
        &self,
        fields: &T,
    ) -> Result<Vec<Option<Value>>, ErrorAtLocation<ValidationError, usize>> {
        let mut rv = Vec::with_capacity(self.columns.len());
        let fields = fields.as_ref();
        for (idx, (col, value)) in self.columns.iter().zip(fields.iter()).enumerate() {
            match col.validate_value(value) {
                Err(e) => {
                    let col_idx = idx + 1;

                    return Err(ValidationError::ValueError(e).at(col_idx));
                }
                Ok(v) => rv.push(v),
            }
        }
        Ok(rv)
    }

    pub fn read_field<T: AsRef<[String]>>(
        &self,
        fields: &T,
        idx: usize,
    ) -> Result<Option<Value>, ValidationError> {
        let col = self.columns.get(idx).ok_or(ValidationError::SchemaMismatch)?;
        let raw = fields.as_ref().get(idx).ok_or(
            ValidationError::SchemaMismatch,
        )?;

        let field = col.validate_value(raw)?;
        Ok(field)
    }

    pub fn read_field_by_name<T: AsRef<[String]>>(
        &self,
        fields: &T,
        name: &str,
    ) -> Result<Option<Value>, ValidationError> {
        let idx = self.col_idx(name).ok_or(ValidationError::SchemaMismatch)?;
        self.read_field(fields, idx)
    }
}


#[inline]
fn cap<T>(c: &regex::Captures, idx: usize) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    c.get(idx)
        .safe_unwrap("valid group")
        .as_str()
        .parse()
        .safe_unwrap("already validated through regex")

}

pub fn parse_filename<S: AsRef<str>>(filename: S) -> Option<CsvxMetadata> {
    match FN_RE.captures(filename.as_ref()) {
        Some(caps) => {
            let table_name = caps.get(1).safe_unwrap("known group").as_str().to_string();
            let year = cap(&caps, 2);
            let month = cap(&caps, 3);
            let day = cap(&caps, 4);
            let schema = caps.get(5).safe_unwrap("known group").as_str().to_string();

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
        assert_eq!(
            parse_filename("zoo-nyc_20170401_animals-2.csv").unwrap(),
            CsvxMetadata {
                table_name: "zoo-nyc".to_owned(),
                date: NaiveDate::from_ymd(2017, 04, 01),
                schema: "animals-2".to_owned(),
            }
        );
    }

}
