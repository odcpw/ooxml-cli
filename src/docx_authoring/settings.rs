pub(super) fn settings_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:updateFields w:val="false"/></w:settings>"#
}

pub(super) fn font_table_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:font w:name="Aptos"><w:altName w:val="Calibri"/><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font><w:font w:name="Aptos Display"><w:altName w:val="Calibri Light"/><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font><w:font w:name="Calibri"><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font><w:font w:name="Arial"><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font><w:font w:name="Liberation Sans"><w:family w:val="swiss"/><w:pitch w:val="variable"/></w:font></w:fonts>"#
}
