# Import and export

**Import** accepts GEDCOM 5.5, 5.5.1 and 7.0 in UTF-8, UTF-16, ANSEL, ISO-8859-1 and Windows-1252. The importer never refuses a file: damaged lines, level jumps, dangling pointers, duplicate IDs and unreadable dates are recorded in the log and the rest is imported. Data KinTree does not model (vendor extensions such as `_UID`, `_MILT`, `_FSFTID`) is preserved and re-exported unchanged.

**Export** options: GEDCOM 5.5.1 or 7.0; UTF-8, UTF-16, ISO-8859-1 or ASCII (lossy for characters the set cannot hold); living people included, **masked** ("Living" with sex only) or **removed**; a target-software identification in the header. A person counts as living unless they have a death/burial/cremation event or were born more than 110 years ago; you can override this per person.

## Spreadsheets, JSON and calendars
* **People as CSV** – one row per person (id, given, surname, sex, birth/death date and place, occupation, father/mother/partner ids). Opens in any spreadsheet. Text that could be mistaken for a spreadsheet formula is neutralised.
* **Import people from CSV** – columns are recognised by name in English or Turkish (*Ad*, *Soyad*, *Cinsiyet*, *Doğum Tarihi*, *Doğum Yeri*, *Meslek* …); commas or semicolons; quoted fields; a BOM. If `id`, `father_id`, `mother_id` or `partner_ids` columns are present the families are rebuilt. Rows are never rejected: problems (no name, unreadable date) are listed as warnings and the text is kept.
* **Project as JSON** – every table of the project in one file, for backups or other tools.
* **Calendar (.ics)** – see *Timeline and calendar*.

## Websites, archives and encryption
* **Static website** – a ZIP of plain HTML: an index with search plus one page per person (events, relatives, thumbnails). Choose English or Türkçe and whether living people are excluded, shown as “Living”, or included.
* **GEDZIP** – a GEDCOM with all embedded photos and documents. Importing a `.gdz`/`.zip` re-attaches the files to the imported media items by file name.
* **Encrypted backup** – the whole project, media included, protected by a password (Argon2id key derivation, ChaCha20-Poly1305). There is no recovery: keep the password safe.

## Gramps and combining trees
* **Gramps XML** (`.gramps`, compressed or plain) imports from the same file box as GEDCOM: people, names, events with dates and places (coordinates included), families, sources, citations and notes.
* Importing into a project that already has people **adds** to it. Afterwards open **Data quality → Duplicates** to merge people who appear in both files.
