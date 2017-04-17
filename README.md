csvx (Version 1)
================

**csvx** is an attempt to define a precise CSV-based specification with no ambiguities to exchange table-based data. In goes further than [RFC4180](https://tools.ietf.org/html/rfc4180), but every file that conforms to csvx should also conform to RFC4180.


Basics
------

* An **identifier** shall be a string matching the following regular expression: `[a-z][a-z0-9]*`.
* An **indentifier with underscores** shall be a string matching the following regular expression: `[a-z][a-z0-9_]`.
* A **date string** shall be a string matching the following regular expression: `\d{4}\d{2}\d{2}`, with the digit groups denoting *year, month, date* in exactly that order.
* An **integer string** shall be a string matching the following regular expression: `(0|[1-9][0-9]*)`, to be interpreted as a base 10 integer.


Metadata
--------

Each file shall be named using according to the following structure: `tablename_date_schema-schemaversion_csvxversion.csvx`.

* `tablename` shall be arbitrary *identifier*.
* `date` shall be a *date string*, denoting a date associated with the file (*e.g.* the export date of the data within).
* `schema` shall be an *identifier* for the schema used (see below).
* `schemaversion` shall be an *integer string*, denoting the version of the `schema`.
* `csvxversion` shall be an *integer string* denoting the csvx version used.

### Example

A csvx export of a zoo's animal database, exported on April 17th, 2017, using the `animals` schema, version 3, being exported as csvx version 1 should have the following filename:

`all_20170417_animals-3_1.csvx`

(`all` has been chose as the `tablename` portion).


Format and encoding
-------------------

* All csvx files shall be UTF8 encoded, with no byte order mark, normalized to NFC.
* Lines shall be terminated by `\r\n` (carriage return, line feed)
* The file shall end on `\r\n`
* There must not be any empty lines.


Compression
-----------

csvx files may be compressed using either [gzip](https://tools.ietf.org/html/rfc1952) or [xz](http://tukaani.org/xz/xz-file-format.txt) compression. The compression used is denoted by appending `.gzip` or `.xz` to the filename, respectively.
