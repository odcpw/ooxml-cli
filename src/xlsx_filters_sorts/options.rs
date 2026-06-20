pub(crate) struct XlsxFiltersSortsSetAutoFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) expect_range: Option<&'a str>,
    pub(crate) expect_range_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsClearAutoFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) expect_range: Option<&'a str>,
    pub(crate) expect_range_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsAddColumnFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) column: i64,
    pub(crate) values: Option<&'a str>,
    pub(crate) custom_op: Option<&'a str>,
    pub(crate) custom_val1: Option<&'a str>,
    pub(crate) custom_val2: Option<&'a str>,
    pub(crate) custom_present: bool,
    pub(crate) expect_filter: Option<&'a str>,
    pub(crate) expect_filter_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsClearColumnFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) column: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsSetSortOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) ref_range: Option<&'a str>,
    pub(crate) column: Option<&'a str>,
    pub(crate) descending: bool,
    pub(crate) expect_sort: Option<&'a str>,
    pub(crate) expect_sort_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsClearSortOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone, Copy)]
pub(super) struct XlsxFiltersSortsOutputOptions<'a> {
    pub(super) out: Option<&'a str>,
    pub(super) backup: Option<&'a str>,
    pub(super) dry_run: bool,
    pub(super) no_validate: bool,
    pub(super) in_place: bool,
}
