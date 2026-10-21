Bundled fonts (SIL Open Font License 1.1, see the `*-LICENSE.txt` files):

* Inter 4.1 (https://github.com/rsms/inter)
* JetBrains Mono 2.304 (https://github.com/JetBrains/JetBrainsMono)

They are subset to drop the Unicode private-use area, so the Phosphor icon font (which lives
there) is not shadowed:

    pyftsubset X.ttf --unicodes="U+0000-D7FF,U+F900-FFFF" --layout-features='*' --name-IDs='*' --notdef-outline
