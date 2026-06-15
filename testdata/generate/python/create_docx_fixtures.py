#!/usr/bin/env python3
"""Generate minimal DOCX fixtures for ooxml-cli tests.

These use only Python's standard library so DOCX fixture generation does not
depend on Word, LibreOffice, or third-party document packages.
"""

from __future__ import annotations

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docx"


CONTENT_TYPES_BASE = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  {defaults}
  {overrides}
</Types>
"""

ROOT_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"""

DOC_START = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
"""

DOC_END = """    <w:sectPr/>
  </w:body>
</w:document>
"""


def write_package(path: Path, parts: dict[str, str | bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(path, "w", ZIP_DEFLATED) as zf:
        for name, data in parts.items():
            zf.writestr(name, data)


def content_types(overrides: str, defaults: str = "") -> str:
    return CONTENT_TYPES_BASE.format(defaults=defaults.strip(), overrides=overrides.strip())


def document_override(extra: str = "") -> str:
    return f"""
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
{extra}"""


def minimal() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/document.xml": DOC_START
        + """    <w:p><w:r><w:t>Hello world</w:t></w:r></w:p>
"""
        + DOC_END,
    }
    write_package(OUT / "minimal" / "document.docx", parts)


def styled_headings() -> None:
    extra = """
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"""
    parts = {
        "[Content_Types].xml": content_types(document_override(extra)),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>
""",
        "word/document.xml": DOC_START
        + """    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Heading Text</w:t></w:r></w:p>
    <w:p><w:r><w:t>Body text</w:t></w:r></w:p>
"""
        + DOC_END,
        "word/styles.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>
</w:styles>
""",
    }
    write_package(OUT / "styled-headings" / "document.docx", parts)


def table() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/document.xml": DOC_START
        + """    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
"""
        + DOC_END,
    }
    write_package(OUT / "table" / "document.docx", parts)


def merged_table() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/document.xml": DOC_START
        + """    <w:tbl>
      <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>Merged</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
"""
        + DOC_END,
    }
    write_package(OUT / "merged-table" / "document.docx", parts)


def hyperlink() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
</Relationships>
""",
        "word/document.xml": DOC_START
        + """    <w:p><w:r><w:t>Before </w:t></w:r><w:hyperlink r:id="rLink"><w:r><w:t>link text</w:t></w:r></w:hyperlink><w:r><w:t> after</w:t></w:r></w:p>
"""
        + DOC_END,
    }
    write_package(OUT / "hyperlink" / "document.docx", parts)


def space_preserve() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/document.xml": DOC_START
        + """    <w:p><w:r><w:t xml:space="preserve"> pad </w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>tabbed</w:t></w:r><w:r><w:br/></w:r><w:r><w:t>line</w:t></w:r></w:p>
"""
        + DOC_END,
    }
    write_package(OUT / "space-preserve" / "document.docx", parts)


def with_media() -> None:
    extra = """
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"""
    defaults = """<Default Extension="png" ContentType="image/png"/>"""
    parts: dict[str, str | bytes] = {
        "[Content_Types].xml": content_types(document_override(extra), defaults),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
</Relationships>
""",
        "word/document.xml": DOC_START
        + """    <w:p><w:r><w:t>Image placeholder</w:t></w:r></w:p>
"""
        + DOC_END,
        "word/header1.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Header</w:t></w:r></w:p></w:hdr>
""",
        "word/footer1.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Footer</w:t></w:r></w:p></w:ftr>
""",
        "word/media/image1.png": b"\x89PNG\r\n\x1a\n",
    }
    write_package(OUT / "with-media" / "document.docx", parts)


def headers() -> None:
    extra = """
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"""
    parts: dict[str, str | bytes] = {
        "[Content_Types].xml": content_types(document_override(extra)),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId10" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rId11" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
</Relationships>
""",
        "word/document.xml": DOC_START
        + """    <w:p><w:r><w:t>Body paragraph</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rId10"/>
      <w:footerReference w:type="default" r:id="rId11"/>
    </w:sectPr>
  </w:body>
</w:document>
""",
        "word/header1.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Page Header</w:t></w:r></w:p></w:hdr>
""",
        "word/footer1.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Page Footer</w:t></w:r></w:p></w:ftr>
""",
    }
    write_package(OUT / "headers" / "document.docx", parts)


def corrupted_missing_document() -> None:
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
    }
    write_package(OUT / "corrupted-missing-document" / "document.docx", parts)


def mixed_blocks() -> None:
    extra = """
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"""
    parts = {
        "[Content_Types].xml": content_types(document_override(extra)),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>
""",
        "word/document.xml": DOC_START
        + """    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>Cell text</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Bold heading</w:t></w:r></w:p>
    <w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>Paragraph with section props</w:t></w:r></w:p>
    <w:p><w:r><w:t>Tail paragraph</w:t></w:r></w:p>
"""
        + DOC_END,
        "word/styles.xml": """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>
</w:styles>
""",
    }
    write_package(OUT / "mixed-blocks" / "document.docx", parts)


def default_ns() -> None:
    document = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<document xmlns="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <body>
    <p><r><t>Default namespace text</t></r></p>
    <sectPr/>
  </body>
</document>
"""
    parts = {
        "[Content_Types].xml": content_types(document_override()),
        "_rels/.rels": ROOT_RELS,
        "word/document.xml": document,
    }
    write_package(OUT / "default-ns" / "document.docx", parts)


def main() -> None:
    minimal()
    styled_headings()
    table()
    merged_table()
    hyperlink()
    space_preserve()
    with_media()
    headers()
    corrupted_missing_document()
    mixed_blocks()
    default_ns()


if __name__ == "__main__":
    main()
