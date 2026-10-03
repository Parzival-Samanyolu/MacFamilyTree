# Import and export

**Import** accepts GEDCOM 5.5, 5.5.1 and 7.0 in UTF-8, UTF-16, ANSEL, ISO-8859-1 and Windows-1252. The importer never refuses a file: damaged lines, level jumps, dangling pointers, duplicate IDs and unreadable dates are recorded in the log and the rest is imported. Data KinTree does not model (vendor extensions such as `_UID`, `_MILT`, `_FSFTID`) is preserved and re-exported unchanged.

**Export** options: GEDCOM 5.5.1 or 7.0; UTF-8, UTF-16, ISO-8859-1 or ASCII (lossy for characters the set cannot hold); living people included, **masked** ("Living" with sex only) or **removed**; a target-software identification in the header. A person counts as living unless they have a death/burial/cremation event or were born more than 110 years ago; you can override this per person.
