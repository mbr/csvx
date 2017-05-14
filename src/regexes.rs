use regex::Regex;
use safe_unwrap::SafeUnwrap;


lazy_static! {
    pub static ref IDENT_UNDERSCORE_RE: Regex = Regex::new(
        r"^[a-z][a-z0-9_]*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref ENUM_EXPR_RE: Regex = Regex::new(
        r"^ENUM.*\(((?:[A-Z][A-Z0-9]*,?)*)\)$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref CONSTRAINT_RE: Regex = Regex::new(
        r"^(:?[A-Z]+,?)*$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref DECIMAL_RE: Regex = Regex::new(
        r"^\d+(?:\.\d+)?$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref DATE_RE: Regex = Regex::new(
        r"^(\d{4})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref DATETIME_RE: Regex = Regex::new(
        r"^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    pub static ref TIME_RE: Regex = Regex::new(
        r"^(\d{2})(\d{2})(\d{2})$"
    ).safe_unwrap("built-in Regex is broken. Please file a bug");
}

lazy_static! {
    // `tablename_date_schema-schemaversion_csvxversion.csvx`
    pub static ref FN_RE: Regex = Regex::new(
        r"^([a-z][a-z0-9-]*)_(\d{4})(\d{2})(\d{2})_([a-z][a-z0-9-]*).csv$"
    ).expect("built-in Regex is broken. Please file a bug");
}
