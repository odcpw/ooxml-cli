pub(crate) fn command_arg(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let needs_quotes = value.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t'
                | '\r'
                | '\n'
                | '\''
                | '"'
                | '\\'
                | '$'
                | '`'
                | '<'
                | '>'
                | '|'
                | '&'
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '*'
                | '?'
                | '!'
        )
    });
    if !needs_quotes {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
