use crate::{CliError, CliResult};

pub(super) fn core_properties_xml() -> CliResult<String> {
    let timestamp = source_date_epoch_timestamp()?;
    let dates = timestamp.map_or_else(String::new, |timestamp| {
        format!(
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:modified>"#
        )
    });
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>ooxml-cli</dc:creator><cp:lastModifiedBy>ooxml-cli</cp:lastModifiedBy>{dates}</cp:coreProperties>"#
    ))
}

pub(super) fn app_properties_xml(text: &str) -> String {
    let characters = text.chars().count();
    let words = text.split_whitespace().count();
    let characters_with_spaces = text.chars().count();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Template>Normal.dotm</Template><TotalTime>0</TotalTime><Pages>1</Pages><Words>{words}</Words><Characters>{characters}</Characters><Application>ooxml-cli</Application><DocSecurity>0</DocSecurity><Lines>1</Lines><Paragraphs>1</Paragraphs><ScaleCrop>false</ScaleCrop><Company></Company><LinksUpToDate>false</LinksUpToDate><CharactersWithSpaces>{characters_with_spaces}</CharactersWithSpaces><SharedDoc>false</SharedDoc><HyperlinksChanged>false</HyperlinksChanged><AppVersion>1.0</AppVersion></Properties>"#
    )
}

fn source_date_epoch_timestamp() -> CliResult<Option<String>> {
    let value = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(err) => {
            return Err(CliError::invalid_args(format!(
                "SOURCE_DATE_EPOCH is not valid UTF-8: {err}"
            )));
        }
    };
    let seconds = value
        .parse::<u64>()
        .map_err(|_| CliError::invalid_args("SOURCE_DATE_EPOCH must be a non-negative integer"))?;
    if seconds > 253_402_300_799 {
        return Err(CliError::invalid_args(
            "SOURCE_DATE_EPOCH must represent a UTC instant no later than 9999-12-31T23:59:59Z",
        ));
    }
    Ok(Some(format_unix_timestamp(seconds)))
}

fn format_unix_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::format_unix_timestamp;

    #[test]
    fn formats_source_date_epoch_as_utc() {
        assert_eq!(format_unix_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_timestamp(946_684_800), "2000-01-01T00:00:00Z");
        assert_eq!(format_unix_timestamp(1_709_251_199), "2024-02-29T23:59:59Z");
    }
}
