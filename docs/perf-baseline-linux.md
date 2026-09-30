# Linux dev-machine performance baseline

This is a dev-machine baseline, not the Windows acceptance baseline. The confirmed laptop (4 GB RAM, 2 cores, slow storage) was not available. Numbers below are from one Node 24 process on a warm Linux page cache, after two discarded warmup runs. They do not include Electron process launch, renderer paint, or cold storage.

Measured commit `53897912c609b0d129052acba6d9c9927e50c7ea`. Working tree patch hash `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (empty diff). Raw samples stay in gitignored `.perf/` and are not committed.

## Environment

| Field    | Value                                                                                    |
| -------- | ---------------------------------------------------------------------------------------- |
| CPU      | Intel(R) Xeon(R) Processor, 4 cores                                                      |
| RAM      | 16,791,945,216 bytes total, 4,522,205,184 bytes free when the bench finished             |
| OS       | linux 6.12.94+                                                                           |
| Node     | v24.21.0                                                                                 |
| Electron | package `^44.4.5`; `process.versions.electron` was null because the bench ran under Node |

## Fixtures

Seed `1`. Bodies are composed from `bundled-templates/چالان آپریشن بغیر فرد ملزم جوڈیشل جنرل 1 گواہ.json` (sha256 `ef7171a357f25aedb2f93954a021dcbb9966963ac415681e89fb188996b111e4`), keeping its Urdu text, bidirectional runs, Word font styles, tables, and linked fields. Each document also inserts a bold/underline Urdu-plus-Latin paragraph and catalog fields, including the custom field `خانہ نمبر`. Page count is the number of composed pages separated by lexical `pagebreak` nodes. The source envelope has no images, so image dimensions are empty. Database hashes are sha256 of the main SQLite file after `wal_checkpoint(TRUNCATE)`. Repository timestamps are frozen at `2023-11-14T22:13:20.001Z` so the same seed hashes the same small database.

|                           |                                                              Small |                                                             Normal |
| ------------------------- | -----------------------------------------------------------------: | -----------------------------------------------------------------: |
| FIRs                      |                                                                100 |                                                              5,000 |
| Bundled templates         |                                                                100 |                                                                100 |
| User templates            |                                                                  0 |                                                                100 |
| FIR documents             |                                                                100 |                                                              5,000 |
| Placeholders              |                                                                 28 |                                                                 28 |
| Pages (min / max / total) |                                                        1 / 5 / 572 |                                                    5 / 20 / 63,076 |
| Lexical nodes             |                                                             62,820 |                                                          7,189,026 |
| Field nodes               |                                                              7,672 |                                                            897,260 |
| Composed envelope bytes   |                                                         31,626,638 |                                                      3,631,815,200 |
| Largest envelope bytes    |                                                            431,242 |                                                          1,292,818 |
| Database bytes            |                                                         32,776,192 |                                                      3,725,606,912 |
| Database sha256           | `184a16366f3e3992870ca8136307c0ffd4dade150de729aeeeacb0e40be57bf9` | `57768147d30da8ccad7a1a3a22d54d9bcb505ce6d1e481d465940940af2befd3` |

The stress profile (25,000 FIRs, 100 bundled and 400 user templates, 50–100 pages, 12 extra linked fields per page) is implemented and was not generated.

## Small-fixture repository timings

`vp run perf:bench -- small` against the small database above. Warmup 2. Launch scenario `openMigrate` has 10 samples. The other scenarios have 10 samples. Each sample is one repository call inside an already-loaded process. `openMigrate` copies the sealed database, then times a new runtime through migrations until `FirRepository` is available. `documentSave10` saves one representative 10-page envelope and waits for the acknowledgement.

| Scenario                      |  Median |      p95 |
| ----------------------------- | ------: | -------: |
| openMigrate                   | 1.99 ms |  4.15 ms |
| firList                       | 0.65 ms |  3.41 ms |
| templateList                  | 2.81 ms |  3.15 ms |
| templateSearch (`ریمانڈ`)     | 2.93 ms |  3.03 ms |
| documentGet                   | 1.14 ms |  1.23 ms |
| documentGetMany (8 documents) | 6.96 ms | 11.24 ms |
| documentSave10                | 4.22 ms |  4.52 ms |

Heap and RSS deltas are stored per sample in the raw bench JSON. On this run they move with the payload: `documentGetMany` heap median about 4.4 MB, `documentSave10` heap median about 2.0 MB. Smaller reads sit in the hundreds of kilobytes and are noisy.

The normal database was generated and not timed. Copying a 3.7 GB file for each launch sample would measure disk copy more than migration.

## Fresh-profile template list

`vp run perf:fresh` on commit `e9adf0da9793b5844e19ab37c664979cb51b571c`. Working tree patch hash `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (empty diff). Same machine as the tables above; free RAM when this run finished was 4,262,023,168 bytes. Warmup 2, then 10 samples, alternating a seed open and an installer open. Schema JIT is enabled, matching the storage worker. Each sample creates a new profile, constructs `makeStorageRuntime`, and stops when pack status is `Ready` and `TemplateRepository.list` returns every template. The clock includes seed publication. It does not include Electron process launch or renderer paint. Raw samples are in gitignored `.perf/fresh-profile.json`.

The shipped pack is the one Word template in `bundled-templates/`. The synthetic pack is 100 templates composed the same way as the small fixture (seed `1`, 1–5 pages, one extra linked field per page, Urdu source envelope). Both seeds are built by `scripts/template-pack/seed.ts`. The installer column uses the same bodies and manifest with the seed removed, so the background installer fills an empty database.

| Pack          | Templates | Seed gzip | Uncompressed seed and published database | Seed median | Seed p95 | Installer median | Installer p95 |
| ------------- | --------: | --------: | ---------------------------------------: | ----------: | -------: | ---------------: | ------------: |
| Shipped       |         1 |  35,422 B |                                 974,848 B |    24.71 ms | 29.06 ms |         34.93 ms |      40.55 ms |
| Synthetic 100 |       100 | 560,726 B |                              16,453,632 B |    76.11 ms | 90.66 ms |        385.27 ms |     408.84 ms |

The published database is the same size as the uncompressed seed, which is the closed snapshot copied into the profile. On this warm Linux page cache the 100-template seed reaches a full list in about a fifth of the background installer's time.

## Follow-up mark points

Opt-in marks (`MISSAL_PERF=1`) currently cover main startup (`app.ready`, `window.created`, `window.ready-to-show`, `window.did-finish-load`, `storage.ready`) and storage decode, per-request execute, and encode timings with payload sizes. Still to mark, in code this change does not own:

- Editor capture, typing, undo, and save-while-typing
- DOCX unzip, XML normalization, DOM render, and Lexical conversion
- Print layout, font and image readiness, and PDF rendering
- Bundled-template synchronization phases during storage startup
- Renderer FIR scroll and template command search
