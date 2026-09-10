"""Exercise the installed wheel without private document fixtures."""
import io
import zipfile
from xml.sax.saxutils import escape
from kreuzberg import ExtractionConfig, OutputFormat, extract_bytes_sync

def docx(text):
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, 'w') as z:
        z.writestr('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
        z.writestr('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
        z.writestr('word/document.xml', '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblGrid><w:gridCol w:w="4500"/><w:gridCol w:w="4500"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>'+escape(text)+'</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>SECOND_COLUMN</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>')
    return stream.getvalue()

for text in ['ordinary cell', 'x'*65535, 'x'*65536, 'Условия договора. '*10000+'КОНЕЦЯЧЕЙКИ']:
    result = extract_bytes_sync(docx(text), 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', config=ExtractionConfig(output_format=OutputFormat.MARKDOWN, use_cache=False))
    assert text in result.content, 'Markdown truncated cell'
    assert result.tables and any(text in cell for table in result.tables for row in table.cells for cell in row), 'Structured table truncated cell'
    assert len(result.content) < len(text)+1000, 'Excessive padding'
    print('PASS:',len(text.encode()),'UTF-8 bytes')
