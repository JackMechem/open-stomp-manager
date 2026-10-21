# analysis/ — static reverse-engineering artefacts

Raw outputs of the static analysis of Stomp Manager 1.0.2 (macOS). The write-up is
[`docs/08-static-analysis.md`](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/08-static-analysis.md) (in the open-stomp-lib repository).

Committed: small curated text dumps (struct schema, effect DB, command call sites, dependency list, filtered strings)
and the scripts that produced them (`scripts/`, including `blutter_macos_macho.patch` for running blutter on a macOS
Flutter Mach-O).

Not committed (see `.gitignore`): `extracted/` (thinned app binary, disassembly, firmware, updater, native-lib dumps),
`tools_src/` (patched blutter + Dart VM build, ~133 MB), `.venv/`, and the large object-pool dumps. These are
derived from proprietary binaries and/or are large. Regenerate them locally from your own copy of the app with the
scripts.
