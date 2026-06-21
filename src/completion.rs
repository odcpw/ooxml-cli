use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{CliError, CliResult, EXIT_SUCCESS};

pub(crate) fn completion(args: &[String]) -> CliResult<DispatchOutput> {
    let [shell] = args else {
        return Err(CliError::invalid_args(
            "completion requires exactly one shell: bash, fish, powershell, or zsh",
        ));
    };
    let text = match shell.as_str() {
        "bash" => bash_completion(),
        "fish" => fish_completion(),
        "powershell" => powershell_completion(),
        "zsh" => zsh_completion(),
        other => {
            return Err(CliError::invalid_args(format!(
                "unsupported completion shell: {other} (expected bash, fish, powershell, or zsh)"
            )));
        }
    };
    Ok(DispatchOutput {
        body: DispatchBody::Text(text),
        exit_code: EXIT_SUCCESS,
    })
}

fn commands() -> &'static [&'static str] {
    &[
        "apply",
        "capabilities",
        "completion",
        "conformance",
        "convert",
        "doctor",
        "docx",
        "find",
        "help",
        "inspect",
        "mcp",
        "pptx",
        "robot-docs",
        "serve",
        "validate",
        "vba",
        "verify",
        "version",
        "xlsx",
    ]
}

fn bash_completion() -> String {
    format!(
        r#"# bash completion for ooxml
_ooxml()
{{
    local cur="${{COMP_WORDS[COMP_CWORD]}}"
    local commands="{commands}"
    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    else
        COMPREPLY=()
    fi
}}
complete -F _ooxml ooxml
"#,
        commands = commands().join(" ")
    )
}

fn fish_completion() -> String {
    let mut lines = vec!["# fish completion for ooxml".to_string()];
    for command in commands() {
        lines.push(format!(
            "complete -c ooxml -n '__fish_use_subcommand' -a {command}"
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn powershell_completion() -> String {
    format!(
        r#"# PowerShell completion for ooxml
Register-ArgumentCompleter -Native -CommandName ooxml -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)
    $commands = @({commands})
    $commands |
        Where-Object {{ $_ -like "$wordToComplete*" }} |
        ForEach-Object {{
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }}
}}
"#,
        commands = commands()
            .iter()
            .map(|command| format!("'{command}'"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn zsh_completion() -> String {
    format!(
        r#"#compdef ooxml
# zsh completion for ooxml
_ooxml() {{
  local -a commands
  commands=({commands})
  _describe 'command' commands
}}
compdef _ooxml ooxml
"#,
        commands = commands().join(" ")
    )
}
