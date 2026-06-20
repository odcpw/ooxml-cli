use crate::{WorkbookSheet, add_selector};

#[derive(Clone, Default)]
pub(super) struct XlsxDefinedName {
    pub(super) number: u32,
    pub(super) name: String,
    pub(super) scope: String,
    pub(super) local_sheet_id: Option<i64>,
    pub(super) sheet_number: u32,
    pub(super) sheet_name: String,
    pub(super) ref_text: String,
    pub(super) hidden: bool,
    pub(super) comment: String,
    pub(super) description: String,
    pub(super) primary_selector: String,
    pub(super) selectors: Vec<String>,
}

#[derive(Clone)]
pub(super) struct XlsxDefinedNameSpan {
    pub(super) name: XlsxDefinedName,
}

pub(super) struct XlsxDefinedNamesBlock {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) names: Vec<XlsxDefinedNameSpan>,
}

pub(crate) struct XlsxNameMutationOptions<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) new_name: Option<&'a str>,
    pub(crate) ref_: Option<&'a str>,
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) scope_sheet: Option<&'a str>,
    pub(crate) expect_ref: Option<&'a str>,
    pub(crate) hidden: bool,
    pub(crate) comment: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

impl XlsxDefinedName {
    pub(super) fn apply_selectors(&mut self) {
        self.primary_selector = if self.scope == "workbook" && !self.name.trim().is_empty() {
            format!("name:{}", self.name)
        } else if self.scope == "sheet" && self.sheet_number > 0 && !self.name.trim().is_empty() {
            format!("sheet:{}/name:{}", self.sheet_number, self.name)
        } else if self.number > 0 {
            format!("definedName:{}", self.number)
        } else {
            String::new()
        };

        let mut selectors = Vec::new();
        add_selector(&mut selectors, self.primary_selector.clone());
        if self.number > 0 {
            add_selector(&mut selectors, format!("definedName:{}", self.number));
            add_selector(&mut selectors, format!("#{}", self.number));
        }
        if !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("name:{}", self.name));
            add_selector(&mut selectors, format!("~{}", self.name));
            add_selector(&mut selectors, self.name.clone());
        }
        if self.scope == "workbook" && !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("scope:workbook/name:{}", self.name));
            add_selector(&mut selectors, format!("workbook:{}", self.name));
        }
        if self.scope == "sheet" && !self.name.trim().is_empty() {
            if self.sheet_number > 0 {
                add_selector(
                    &mut selectors,
                    format!("scope:sheet:{}/name:{}", self.sheet_number, self.name),
                );
                add_selector(
                    &mut selectors,
                    format!("sheet:{}/name:{}", self.sheet_number, self.name),
                );
            }
            if !self.sheet_name.trim().is_empty() {
                add_selector(
                    &mut selectors,
                    format!("scope:sheet:{}/name:{}", self.sheet_name, self.name),
                );
                add_selector(
                    &mut selectors,
                    format!("sheet:{}/name:{}", self.sheet_name, self.name),
                );
            }
        }
        self.selectors = selectors;
    }
}

pub(super) fn apply_defined_name_sheet_context(
    name: &mut XlsxDefinedName,
    sheets: &[WorkbookSheet],
) {
    if let Some(local_sheet_id) = name.local_sheet_id {
        name.scope = "sheet".to_string();
        name.sheet_number = if local_sheet_id >= 0 {
            local_sheet_id as u32 + 1
        } else {
            0
        };
        if local_sheet_id >= 0
            && let Some(sheet) = sheets.get(local_sheet_id as usize)
        {
            name.sheet_name = sheet.name.clone();
        }
    } else {
        name.scope = "workbook".to_string();
        name.sheet_number = 0;
        name.sheet_name.clear();
    }
}

pub(super) fn defined_name_scope_text(local_sheet_id: Option<i64>) -> &'static str {
    if local_sheet_id.is_some() {
        "sheet"
    } else {
        "workbook"
    }
}

pub(super) fn renumber_defined_names(names: &mut [XlsxDefinedName]) {
    for (index, name) in names.iter_mut().enumerate() {
        name.number = index as u32 + 1;
        name.apply_selectors();
    }
}
