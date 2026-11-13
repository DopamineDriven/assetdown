file output from running

```log
dopaminedriven@LAPTOP-2IH011V4:~/rust/workspace/crates/xlsxdown-node$ yarn tsx __test__/workup.ts --inspect date1904
PK
!��I��[Content_Types].xml<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>
PK
!ǚ�++
     _rels/.rels<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>
PK
�99xl/workbook.xml<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<workbookPr date1904="1"/>
<sheets><sheet name="Dates" sheetId="1" r:id="rId1"/></sheets>
</workbook>
PK
!̻\���xl/_rels/workbook.xml.rels<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>
PK
xl/styles.xml<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>
<cellXfs count="2"><xf numFmtId="0" fontId="0"/><xf numFmtId="14" fontId="0"/></cellXfs>
</styleSheet>
PK
!�~����xl/worksheets/sheet1.xml<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<dimension ref="A1:A1"/>
<sheetData><row r="1"><c r="A1" s="1"><v>0</v></c></row></sheetData>
</worksheet>
PK

!��I����[Content_Types].xmlPK

!ǚ�++
     ���_rels/.relsPK

�99��4xl/workbook.xmlPK

!̻\������xl/_rels/workbook.xml.relsPK

��~xl/styles.xmlPK

!�~������xl/worksheets/sheet1.xmlPK�
```
