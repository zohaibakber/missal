# Missal

Missal is a desktop writing app for First Information Reports: officers keep FIR records, fill templates, and reuse shared station values.

## Language

**FIR**:
A First Information Report — one police case file with offence, accused, dates, and status.
_Avoid_: case, record, row, document

**Template**:
Reusable FIR document text that contains placeholder tokens to fill later.
_Avoid_: form, letter, layout

**Placeholder**:
A named token in a template, identified by a stable numeric ID and an ASCII key, shown to officers as an Urdu label.
_Avoid_: field, variable, tag, merge field

**Placeholder value**:
The text stored for one placeholder on one FIR, used when that FIR does not already have a matching core field.
_Avoid_: extra value, custom field, overlay

**Settings**:
The single station-wide record of shared placeholder values such as police station and district names.
_Avoid_: preferences, config, profile

**Repository**:
The storage seam React and atoms call; Electron main implements it with SQLite.
_Avoid_: database client, collection, localStorage

**SQLite file**:
The one app database at `userData/missal.sqlite`, reached only from Electron main.
_Avoid_: IndexedDB, browser storage, second database
