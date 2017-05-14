extern crate clap;
extern crate csvx;
extern crate safe_unwrap;
extern crate term_painter;


use clap::{App, Arg, SubCommand};
use safe_unwrap::SafeUnwrap;
use std::{io, path, process};
use term_painter::{Attr, Color, ToStyle};

use csvx::err::{CheckError, ErrorLoc, ErrorAtLocation, HelpPrinter, Location};

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

    let meta = csvx::parse_filename(meta_fn.clone())
        .ok_or_else(|| {
                        CheckError::InvalidCsvxFilename(meta_fn)
                            .at(Location::File(schema_path_s.clone()))
                    })?;

    if !meta.is_schema() {
        return Err(CheckError::NotASchema.at(Location::File(schema_path_s.clone())));
    }

    // load schema
    let schema = csvx::CsvxSchema::from_file(schema_path)
        .map_err(|e| e.convert())?;

    // schema validated correctly, reward user with a checkmark
    println!("{} {}",
             Color::Green.paint(Attr::Bold.paint("✓")),
             Attr::Bold.paint(schema_path_s));

    let mut all_good = true;
    for input_file in input_files {
        // validate filename first.
        // FIXME: should be moved into validation, as filename is validated
        //        and this whole section is a mess!
        let input_fn_s = input_file
            .as_ref()
            .to_owned()
            .file_name()
            .ok_or_else(|| unimplemented!())
            .unwrap()
            .to_string_lossy()
            .to_string();
        let inp_meta = csvx::parse_filename(&input_fn_s)
            .ok_or_else(|| {
                            CheckError::InvalidCsvxFilename(input_fn_s.clone())
                                .at(Location::File(input_fn_s.clone()))
                        })?;

        // FIXME: should not abort just because schema of one file did not
        //        fit
        if inp_meta.schema != meta.table_name {
            return Err(CheckError::SchemaMismatch {
                               schema: meta.table_name.clone(),
                               data: inp_meta.schema.clone(),
                           }
                           .at(Location::File(input_file
                                                  .as_ref()
                                                  .to_string_lossy()
                                                  .to_string())));
        }

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

fn underline(s: &str, c: char) -> String {
    s.chars().map(|_| c).collect()
}

fn cmd_pretty<P: AsRef<path::Path>>(schema_path: P) {
    // FIXME: there should be a common function for this stuff
    // load meta
    let meta_fn = schema_path
        .as_ref()
        .to_owned()
        .file_name()
        .expect("error loading schema - please validate first")
        .to_str()
        .safe_unwrap("already verified UTF8")
        .to_owned();

    let meta = csvx::parse_filename(meta_fn.clone()).expect("error loading schema -
            please validate first");

    // load schema
    let schema = csvx::CsvxSchema::from_file(schema_path).expect("error loading schema -
            please validate first");

    println!("{}\n{}\n\n* {}\n* {}\n\n",
             meta.table_name,
             underline(&meta.table_name, '='),
             meta.date,
             meta.schema);

    for col in schema.iter_columns() {
        match col.ty {
            csvx::ColumnType::Enum(_) => {
                let header = format!("{}: `ENUM`", col.id);
                print!("{}\n{}\n\n* `{}` \n",
                       header,
                       underline(&header, '-'),
                       col.ty);
            }
            _ => {
                let header = format!("{}: `{}`", col.id, col.ty);
                print!("{}\n{}\n\n", header, underline(&header, '-'));
            }
        }

        let cons = format!("{}", col.constraints);
        if cons.len() > 0 {
            print!("* `{}`\n", cons);
        }
        print!("\n{}\n\n\n", col.description);
    }
}

fn main() {
    let app = App::new("csvx")
        .version("5.2.0")
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
                                 .takes_value(true)))
        .subcommand(SubCommand::with_name("pretty")
                        .about("Generate Markdown documentation")
                        .arg(Arg::with_name("schema_path")
                                 .help("Schema to generate documentation for")
                                 .required(true)
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
        Some(ref cmd) if cmd.name == "pretty" => {
            cmd_pretty(cmd.matches
                           .value_of("schema_path")
                           .safe_unwrap("required argument"));
        }
        _ => {
            app.write_help(&mut io::stdout()).unwrap();
            println!();
        }
    };

}
