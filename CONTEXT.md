# Missal

Missal is a desktop writing app for First Information Reports: officers keep FIR records, fill templates, and reuse shared station values.

## Language

**FIR**:
A First Information Report — one police case file with offence, accused, dates, and status.
_Avoid_: case, record, row, document

**Template**:
Reusable Urdu rich text that contains linked fields. Copying a template onto a FIR creates a FIR document; later template edits do not change that copy.
_Avoid_: form, letter, layout

**FIR document**:
One independently edited document attached to a FIR, created by copying a template's body. A FIR can have several. Field values stay linked to the FIR, not to the document body.
_Avoid_: template instance, attachment, file, the FIR itself

**Document envelope**:
The versioned serialized document body stored for a template or FIR document. Callers treat it as data, never as a live editor.
_Avoid_: HTML, CKEditor content, editor state

**Field**:
A catalog token that can appear in a template or FIR document. It has a stable ID, one unique name, and a source binding: a FIR property, a shared setting, or a custom FIR-specific value. Templates refer to it by writing the name between the field markers chosen in Settings (default `@name@`). Renaming it does not change the source binding or existing documents, which store the ID.
_Avoid_: variable, tag, merge field

**Placeholder**:
The stored catalog row for a Field. Storage, IPC, and seeds still use this name.
_Avoid_: extra value, overlay (those are FIR-specific field values)

**Placeholder value**:
The text stored for one custom or shared-setting field on one FIR.
_Avoid_: extra value, custom field, overlay

**Settings**:
The single station-wide record of shared field values such as police station and district names.
_Avoid_: preferences, config, profile

**Repository**:
The storage seam React and atoms call; Electron main implements it with SQLite.
_Avoid_: database client, collection, localStorage

**SQLite file**:
The one app database at `userData/missal.sqlite`, reached only from Electron main.
_Avoid_: IndexedDB, browser storage, second database
