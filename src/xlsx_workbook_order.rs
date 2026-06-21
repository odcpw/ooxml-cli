use quick_xml::Reader;
use quick_xml::events::Event;

use crate::local_name;

pub(crate) fn xlsx_workbook_child_order(local_name: &str) -> i32 {
    match local_name {
        "fileVersion" => 10,
        "fileSharing" => 20,
        "workbookPr" => 30,
        "workbookProtection" => 40,
        "bookViews" => 50,
        "sheets" => 60,
        "functionGroups" => 70,
        "externalReferences" => 80,
        "definedNames" => 90,
        "calcPr" => 100,
        "oleSize" => 110,
        "customWorkbookViews" => 120,
        "pivotCaches" => 130,
        "smartTagPr" => 140,
        "smartTagTypes" => 150,
        "webPublishing" => 160,
        "fileRecoveryPr" => 170,
        "webPublishObjects" => 180,
        "extLst" => 190,
        _ => 10000,
    }
}

pub(crate) fn xlsx_workbook_ordered_insert_position(
    workbook_xml: &str,
    child_local_name: &str,
) -> Option<usize> {
    let target_order = xlsx_workbook_child_order(child_local_name);
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 1
                    && xlsx_workbook_child_order(local_name(e.name().as_ref())) > target_order
                {
                    return Some(start);
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 1
                    && xlsx_workbook_child_order(local_name(e.name().as_ref())) > target_order
                {
                    return Some(start);
                }
            }
            Ok(Event::End(_)) => {
                if depth == 1 {
                    return Some(start);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(crate) fn insert_xlsx_workbook_child_ordered(
    workbook_xml: &str,
    child_local_name: &str,
    child_xml: &str,
) -> Option<String> {
    let insert_at = xlsx_workbook_ordered_insert_position(workbook_xml, child_local_name)?;
    let mut out = String::with_capacity(workbook_xml.len() + child_xml.len());
    out.push_str(&workbook_xml[..insert_at]);
    out.push_str(child_xml);
    out.push_str(&workbook_xml[insert_at..]);
    Some(out)
}
