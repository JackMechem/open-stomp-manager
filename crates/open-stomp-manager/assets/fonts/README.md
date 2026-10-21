Bundled fonts (SIL Open Font License 1.1, see the `*-LICENSE.txt` files):

* Inter 4.1 (https://github.com/rsms/inter), the only typeface in the app

They are subset to drop the Unicode private-use area, so the Phosphor icon font (which lives
there) is not shadowed, and Inter's tabular figures (`tnum`) are frozen in as the default digits,
because egui cannot switch OpenType features on. Equal-width digits keep changing numbers
(knob values, BPM, timestamps) from jittering:

    pyftsubset X.ttf --unicodes="U+0000-D7FF,U+F900-FFFF" --layout-features='*' --name-IDs='*' --notdef-outline
    pyftfeatfreeze -f tnum X.subset.ttf X.ttf     # from opentype-feature-freezer
