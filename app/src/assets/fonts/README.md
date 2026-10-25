Bundled fonts (SIL Open Font License 1.1, see `Inter-LICENSE.txt`):

* Inter 4.1 (https://github.com/rsms/inter), the only typeface in the app

They are subset to drop the Unicode private-use area, and Inter's tabular figures (`tnum`) are frozen in
as the default digits, so changing numbers (knob values, BPM, timestamps) don't jitter even where the
`font-feature-settings` in `global.css` isn't applied:

    pyftsubset X.ttf --unicodes="U+0000-D7FF,U+F900-FFFF" --layout-features='*' --name-IDs='*' --notdef-outline
    pyftfeatfreeze -f tnum X.subset.ttf X.ttf     # from opentype-feature-freezer
