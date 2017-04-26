csvx (Version 4)
================

**csvx** is an attempt to define a precise CSV-based specification with no ambiguities to exchange table-based data. In goes further than [RFC4180](https://tools.ietf.org/html/rfc4180), but every file that conforms to csvx should also conform to RFC4180.


Basics
------

* An **identifier** shall be a string matching the following regular expression: `[a-z][a-z0-9]*`.
* An **uppercase identifier** shall be a string matching the following regular expression: `[A-Z][A-Z0-9]*`
* An **identifier with underscores** shall be a string matching the following regular expression: `[a-z][a-z0-9\-]`.
* An **identifier with hyphens** shall be a string matching the following regular expression: `[a-z][a-z0-9_]`.
* A **date string** shall be a string matching the following regular expression: `\d{4}\d{2}\d{2}`, with the digit groups denoting *year, month, date* in exactly that order.
* An **integer string** shall be a string matching the following regular expression: `(0|[1-9][0-9]*)`, to be interpreted as a base 10 integer.


Metadata
--------

Each file shall be named using according to the following structure: `tablename_date_schema_csvxversion.csv`.

* `tablename` shall be arbitrary *identifier with hyphens*, other than "schema", which is reserved.
* `date` shall be a *date string*, denoting a date associated with the file (*e.g.* the export date of the data within).
* `schema` shall be an *identifier* for the schema used (see below).
* `csvxversion` shall be an *integer string* denoting the csvx version used.

### Example

A csvx export of a zoo's animal database (called `all`), exported on April 17th, 2017, using the `animals-2` schema, being exported as csvx version 4 should have the following filename:

`all_20170417_animals-2_4.csv`


Format and encoding
-------------------

* All csvx files shall be UTF8 encoded, with no byte order mark, normalized to NFC.
* Lines shall be terminated by `\r\n` (carriage return, line feed)
* Fields shall be separated by commas (`,`)
* Fields shall be quoted and escaped according to [RFC4180](https://tools.ietf.org/html/rfc4180). Minimal quotation must be used, e.g. fields that do not contain line breaks, quotation marks or commas shall not be quoted.
* The file shall end on `\r\n`
* There must not be any empty lines
* The first line of each file must be a header line. All its fields must be limited to *identifiers with underscores*.
* The number of fields in each line must be the same as every other line

Every line inside the file may be referred to as a **row**, while the sequence of every nth field of all rows may be referred to as a **column**. The element of such a sequence is a **column header**.


Compression
-----------

csvx files may be compressed using either [gzip](https://tools.ietf.org/html/rfc1952) or [xz](http://tukaani.org/xz/xz-file-format.txt) compression. The compression used is denoted by appending `.gzip` or `.xz` to the filename, respectively.


csvx schemas
------------

In addition, a document with a schema of `csvx-schema` denotes a csvx schema, specifing rules and types for columns. A *column* is identified by its *header*. The following *column headers* make up the *header* *row* in a schema file, with the following column contents:

* `id`: An *identifier with underscores*, unique among columns
* `type`: One of (`STRING`, `INTEGER`, `ENUM(...)`, `DECIMAL`, `DATE`, `DATETIME`, `TIME`). The `...` is a comma-separated list of uppercase identifiers.
* `constraints`: A string containing any of the following, separated by spaces: (`UNIQUE`, `NULLABLE`)
* `description`: A string with a description of the typical contents of the field, intended for human consumption.

### Data types

In general, empty cells are not allowed unless `NULLABLE` is found in `constraints`. If set, an empty cell is considered to be of the special value `NULL` when empty.

* `STRING`: An arbitrary string.
* `BOOL`: A boolean value, either `TRUE` or `FALSE`.
* `INTEGER`: A base 10, signed, 64-Bit integer, with no leading zeroes.
* `ENUM(VAR1,VAR2,...)`: Any literal `VAR1`, `VAR2`, ...
* `DECIMAL`: A base 10 floating point number of arbitrary precision (it is up to the reader to decide how many decimal places to keep). The only non-digit character allowed is the decimal point `.`, at most once.
* `DATE`: An 8-digit date, in the form of `YYYYmmDD`.
* `DATETIME`: A 14-digit timestamp, in the form of `YYYYmmDDHHMMSS`
* `TIME`: A 6-digit time, in the form of `HHMMSS`.


### Example

An example schema for a zoo could look like this:

`animals-2_20170101_csvx-schema_4.csv`

```
id,type,constraints,description
id,INTEGER,UNIQUE,Internal zoo id
name,STRING,,Name of the animal
birthday,DATE,,The animals birthday
weight,INTEGER,,Weight in g
class,ENUM(MAMMAL,BIRD,REPTILE,INSECT),,Class of species
species,STRING,,Species
yearly_food_cost,DECIMAL,,Last year's cost for feed for animal
caretaker,STRING,NULLABLE,Designated caretaker. May be empty if none assigned.
```

A valid data file for this schema:

`zoo-nyc_20170401_animals-2_4.csv`

```
id,name,birthday,weight,class,species,yearly_food_cost,caretaker
1,Brian,20141125,160000,MAMMAL,Gorilla,5000.00,Sam
2,Pinky,19991209,80,MAMMAL,Lab mouse,25.00,
```
