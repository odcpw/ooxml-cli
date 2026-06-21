mod docx;
mod xlsx;

use serde_json::{Value, json};

use crate::capabilities;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult, EXIT_SUCCESS, GlobalFlags};
use crate::inspect::inspect;
use crate::pptx_mutation::*;
use crate::pptx_readback::*;
use crate::pptx_render::pptx_render;
use crate::validation::validate;
use crate::vba::*;
use crate::verify::verify;
use crate::{
    PptxScaffoldOptions, apply, command_arg, diff, diff_command, pptx_diff_command,
    pptx_diff_dispatch, pptx_media_add, pptx_media_list, pptx_media_replace, pptx_scaffold,
    pptx_template_capture, pptx_template_compile, pptx_template_inspect, pptx_translate_apply,
    pptx_translate_export, pptx_validate_layout, pptx_xlsx_bindings_apply, pptx_xlsx_bindings_plan,
    repair_normalize, template_apply, template_profile_inspect, template_profile_save,
    template_tokens,
};

pub(crate) enum DispatchBody {
    Json(Value),
    Text(String),
}

pub(crate) struct DispatchOutput {
    pub(crate) body: DispatchBody,
    pub(crate) exit_code: i32,
}

pub(crate) fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    if crate::help::is_help_request(args) {
        return crate::help::help(args);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "doctor"
    {
        return crate::doctor::doctor(flags, rest);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "find"
    {
        return crate::find::find(flags, rest);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "robot-docs"
    {
        return crate::robot_docs::robot_docs(flags, rest);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "agent"
    {
        return crate::robot_docs::agent_alias(flags, rest);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "completion"
    {
        return crate::completion::completion(rest);
    }
    if let [cmd, rest @ ..] = args
        && cmd == "conformance"
    {
        return crate::conformance::conformance(flags, rest);
    }
    if let [cmd, baseline, candidate, rest @ ..] = args
        && cmd == "diff"
    {
        let output = diff_command(flags, baseline, candidate, rest)?;
        return Ok(DispatchOutput {
            body: DispatchBody::Json(output.value),
            exit_code: output.exit_code,
        });
    }
    if let [family, verb, baseline, candidate, rest @ ..] = args
        && family == "pptx"
        && verb == "diff"
    {
        let output = pptx_diff_dispatch(flags, baseline, candidate, rest)?;
        return Ok(DispatchOutput {
            body: DispatchBody::Json(output.value),
            exit_code: output.exit_code,
        });
    }
    if let [family, verb, file, rest @ ..] = args
        && family == "vba"
        && verb == "office-check"
    {
        reject_unknown_flags(rest, &["--out-dir"], &[])?;
        let out_dir = parse_string_flag(rest, "--out-dir")?;
        let (value, exit_code) = vba_office_check(file, out_dir.as_deref())?;
        return Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code,
        });
    }
    dispatch_value(flags, args).map(|value| DispatchOutput {
        body: DispatchBody::Json(value),
        exit_code: EXIT_SUCCESS,
    })
}

fn dispatch_value(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, rest @ ..] if cmd == "capabilities" => capabilities::capabilities(rest),
        [cmd, file, rest @ ..] if cmd == "apply" => apply(file, rest),
        [cmd, conversion, file, rest @ ..] if cmd == "convert" && conversion == "xlsm-to-xlsx" => {
            convert_xlsm_to_xlsx(file, rest)
        }
        [cmd, verb, file, rest @ ..] if cmd == "repair" && verb == "normalize" => {
            repair_normalize(file, rest)
        }
        [cmd, verb, file, rest @ ..] if cmd == "template" && verb == "apply" => {
            template_apply(file, rest)
        }
        [cmd, verb, file, rest @ ..] if cmd == "template" && verb == "tokens" => {
            template_tokens(file, rest)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "template" && group == "profile" && verb == "save" =>
        {
            template_profile_save(file, rest)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "template" && group == "profile" && verb == "inspect" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            template_profile_inspect(file)
        }
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, baseline, candidate, rest @ ..] if cmd == "diff" => diff(baseline, candidate, rest),
        [cmd, rest @ ..] if cmd == "validate" => {
            let (file, strict) = parse_validate_args(rest, flags.strict)?;
            validate(file, strict)
        }
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
        [family, verb, file] if family == "vba" && verb == "inspect" => vba_inspect(file),
        [family, verb, rest @ ..] if family == "vba" && verb == "build-bin" => {
            reject_unknown_flags(rest, &["--family", "--source", "--out"], &["--force"])?;
            let family = parse_string_flag(rest, "--family")?;
            let sources = parse_string_flags(rest, "--source")?;
            let out = parse_string_flag(rest, "--out")?
                .ok_or_else(|| CliError::invalid_args("--out is required"))?;
            vba_build_bin(VbaBuildBinOptions {
                family: family.as_deref(),
                sources,
                out: &out,
                force: has_flag(rest, "--force"),
            })
        }
        [family, verb, rest @ ..] if family == "vba" && verb == "create" => {
            let value_flags = [
                "--family",
                "--source",
                "--extract-bin",
                "--office-create-script",
                "--out",
                "--backup",
            ];
            let bool_flags = [
                "--enable-vba-object-model-access",
                "--visible",
                "--force",
                "--pure",
                "--dry-run",
                "--no-validate",
                "--in-place",
            ];
            reject_unknown_flags(rest, &value_flags, &bool_flags)?;
            let positionals = positional_args(rest, &value_flags, &bool_flags)?;
            if positionals.len() != 1 {
                return Err(CliError::invalid_args(
                    "vba create requires exactly one positional path",
                ));
            }
            let family = parse_string_flag(rest, "--family")?;
            let sources = parse_string_flags(rest, "--source")?;
            if has_flag(rest, "--pure") {
                if parse_string_flag(rest, "--extract-bin")?.is_some()
                    || parse_string_flag(rest, "--office-create-script")?.is_some()
                    || has_flag(rest, "--enable-vba-object-model-access")
                    || has_flag(rest, "--visible")
                    || has_flag(rest, "--force")
                {
                    return Err(CliError::invalid_args(
                        "--pure cannot be combined with Office-COM create flags (--extract-bin, --office-create-script, --enable-vba-object-model-access, --visible, --force)",
                    ));
                }
                let out = parse_string_flag(rest, "--out")?;
                let backup = parse_string_flag(rest, "--backup")?;
                vba_create_pure(
                    positionals[0],
                    VbaPureCreateOptions {
                        family: family.as_deref(),
                        sources,
                        mutation: VbaMutationOptions {
                            out: out.as_deref(),
                            backup: backup.as_deref(),
                            dry_run: has_flag(rest, "--dry-run"),
                            no_validate: has_flag(rest, "--no-validate"),
                            in_place: has_flag(rest, "--in-place"),
                        },
                    },
                )
            } else {
                if parse_string_flag(rest, "--out")?.is_some()
                    || parse_string_flag(rest, "--backup")?.is_some()
                    || has_flag(rest, "--dry-run")
                    || has_flag(rest, "--no-validate")
                    || has_flag(rest, "--in-place")
                {
                    return Err(CliError::invalid_args(
                        "--out, --backup, --dry-run, --no-validate, and --in-place are supported by vba create --pure; legacy vba create uses the positional output path",
                    ));
                }
                let extract_bin = parse_string_flag(rest, "--extract-bin")?;
                let office_create_script = parse_string_flag(rest, "--office-create-script")?;
                vba_create(
                    positionals[0],
                    VbaCreateOptions {
                        family: family.as_deref(),
                        sources,
                        extract_bin: extract_bin.as_deref(),
                        office_create_script: office_create_script.as_deref(),
                        enable_vba_object_model_access: has_flag(
                            rest,
                            "--enable-vba-object-model-access",
                        ),
                        visible: has_flag(rest, "--visible"),
                        force: has_flag(rest, "--force"),
                    },
                )
            }
        }
        [family, verb, rest @ ..] if family == "vba" && verb == "rebuild" => {
            let value_flags = ["--family", "--source-dir", "--out", "--backup"];
            let bool_flags = ["--dry-run", "--no-validate", "--in-place"];
            reject_unknown_flags(rest, &value_flags, &bool_flags)?;
            let positionals = positional_args(rest, &value_flags, &bool_flags)?;
            if positionals.len() != 1 {
                return Err(CliError::invalid_args(
                    "vba rebuild requires exactly one positional package path",
                ));
            }
            let family = parse_string_flag(rest, "--family")?;
            let source_dir = parse_string_flag(rest, "--source-dir")?
                .ok_or_else(|| CliError::invalid_args("--source-dir is required"))?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_rebuild(
                positionals[0],
                VbaRebuildOptions {
                    family: family.as_deref(),
                    source_dir: &source_dir,
                    mutation: VbaMutationOptions {
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        no_validate: has_flag(rest, "--no-validate"),
                        in_place: has_flag(rest, "--in-place"),
                    },
                },
            )
        }
        [family, verb, bin_path, rest @ ..] if family == "vba" && verb == "inspect-bin" => {
            reject_unknown_flags(rest, &["--family"], &[])?;
            let family = parse_string_flag(rest, "--family")?.ok_or_else(|| {
                CliError::invalid_args("--family is required for inspect-bin (pptx or xlsx)")
            })?;
            vba_inspect_bin(bin_path, &family)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "list" => {
            reject_unknown_flags(rest, &[], &[])?;
            vba_list(file)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "extract" => {
            reject_unknown_flags(rest, &["--out-dir", "--module"], &[])?;
            let out_dir = parse_string_flag(rest, "--out-dir")?
                .ok_or_else(|| CliError::invalid_args("--out-dir is required"))?;
            let selector = parse_string_flag(rest, "--module")?;
            vba_extract(file, &out_dir, selector.as_deref())
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "add-module" => {
            reject_unknown_flags(
                rest,
                &[
                    "--source",
                    "--name",
                    "--kind",
                    "--expect-module-count",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-experimental-vba-source-rewrite",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let source = parse_string_flag(rest, "--source")?
                .ok_or_else(|| CliError::invalid_args("--source is required"))?;
            let name = parse_string_flag(rest, "--name")?;
            let kind = parse_string_flag(rest, "--kind")?;
            let expect_module_count = parse_string_flag(rest, "--expect-module-count")?
                .map(|value| {
                    value.parse::<usize>().map_err(|_| {
                        CliError::invalid_args("--expect-module-count must be an integer")
                    })
                })
                .transpose()?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_add_module(
                file,
                VbaAddModuleOptions {
                    source: &source,
                    name: name.as_deref(),
                    kind: kind.as_deref(),
                    expect_module_count,
                    allow_experimental_vba_source_rewrite: has_flag(
                        rest,
                        "--allow-experimental-vba-source-rewrite",
                    ),
                    mutation: VbaMutationOptions {
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        no_validate: has_flag(rest, "--no-validate"),
                        in_place: has_flag(rest, "--in-place"),
                    },
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "replace-module" => {
            reject_unknown_flags(
                rest,
                &[
                    "--module",
                    "--source",
                    "--expect-sha256",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-experimental-vba-source-rewrite",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let module = parse_string_flag(rest, "--module")?
                .ok_or_else(|| CliError::invalid_args("--module is required"))?;
            let source = parse_string_flag(rest, "--source")?
                .ok_or_else(|| CliError::invalid_args("--source is required"))?;
            let expect_sha256 = parse_string_flag(rest, "--expect-sha256")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_replace_module(
                file,
                VbaReplaceModuleOptions {
                    module: &module,
                    source: &source,
                    expect_sha256: expect_sha256.as_deref(),
                    allow_experimental_vba_source_rewrite: has_flag(
                        rest,
                        "--allow-experimental-vba-source-rewrite",
                    ),
                    mutation: VbaMutationOptions {
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        no_validate: has_flag(rest, "--no-validate"),
                        in_place: has_flag(rest, "--in-place"),
                    },
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "remove-module" => {
            reject_unknown_flags(
                rest,
                &["--module", "--expect-sha256", "--out", "--backup"],
                &[
                    "--allow-experimental-vba-source-rewrite",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let module = parse_string_flag(rest, "--module")?
                .ok_or_else(|| CliError::invalid_args("--module is required"))?;
            let expect_sha256 = parse_string_flag(rest, "--expect-sha256")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_remove_module(
                file,
                VbaRemoveModuleOptions {
                    module: &module,
                    expect_sha256: expect_sha256.as_deref(),
                    allow_experimental_vba_source_rewrite: has_flag(
                        rest,
                        "--allow-experimental-vba-source-rewrite",
                    ),
                    mutation: VbaMutationOptions {
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        no_validate: has_flag(rest, "--no-validate"),
                        in_place: has_flag(rest, "--in-place"),
                    },
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "extract-bin" => {
            reject_unknown_flags(rest, &["--out"], &[])?;
            let out = parse_string_flag(rest, "--out")?
                .ok_or_else(|| CliError::invalid_args("--out is required"))?;
            vba_extract_bin(file, &out)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "attach" => {
            reject_unknown_flags(
                rest,
                &["--bin", "--out", "--backup"],
                &[
                    "--allow-host-family-risk",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let bin = parse_string_flag(rest, "--bin")?
                .ok_or_else(|| CliError::invalid_args("--bin is required"))?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_attach(
                file,
                &bin,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "remove" => {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_remove(
                file,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, ..] if family == "docx" => docx::dispatch_docx(args),
        [family, ..] if family == "xlsx" => xlsx::dispatch_xlsx(args),
        [family, verb, output, rest @ ..] if family == "pptx" && verb == "scaffold" => {
            reject_unknown_flags(
                rest,
                &["--title", "--subtitle"],
                &["--force", "--no-validate"],
            )?;
            let title = parse_string_flag(rest, "--title")?;
            let subtitle = parse_string_flag(rest, "--subtitle")?;
            pptx_scaffold(
                output,
                PptxScaffoldOptions {
                    title: title.as_deref(),
                    subtitle: subtitle.as_deref(),
                    force: has_flag(rest, "--force"),
                    no_validate: has_flag(rest, "--no-validate"),
                },
            )
        }
        [family, verb] if family == "pptx" && verb == "diff" => {
            Err(CliError::invalid_args("accepts 2 arg(s), received 0"))
        }
        [family, verb, _baseline] if family == "pptx" && verb == "diff" => {
            Err(CliError::invalid_args("accepts 2 arg(s), received 1"))
        }
        [family, verb, baseline, candidate, rest @ ..] if family == "pptx" && verb == "diff" => {
            pptx_diff_command(baseline, candidate, rest)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "validate-layout" => {
            reject_unknown_flags(rest, &["--format"], &[])?;
            pptx_validate_layout(file)
        }
        [family, group, verb, manifest, rest @ ..]
            if family == "pptx" && group == "template" && verb == "inspect" =>
        {
            reject_unknown_flags(rest, &["--format"], &[])?;
            pptx_template_inspect(manifest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "template" && verb == "capture" =>
        {
            pptx_template_capture(file, rest)
        }
        [family, group, verb, manifest, spec, rest @ ..]
            if family == "pptx" && group == "template" && verb == "compile" =>
        {
            pptx_template_compile(manifest, spec, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "xlsx-bindings" && verb == "plan" =>
        {
            pptx_xlsx_bindings_plan(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "xlsx-bindings" && verb == "apply" =>
        {
            pptx_xlsx_bindings_apply(file, rest)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "add-textbox" => {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--text",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--name",
                    "--mode",
                    "--font-size",
                    "--font",
                    "--color",
                    "--level",
                    "--align",
                    "--out",
                    "--backup",
                ],
                &[
                    "--bold",
                    "--italic",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_add_textbox(file, rest)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "clone-slide" => {
            reject_unknown_flags(
                rest,
                &["--slide", "--insert-after", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_clone_slide(file, rest)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "new-slide-from-layout" => {
            reject_unknown_flags(
                rest,
                &[
                    "--layout",
                    "--set-text",
                    "--set-rich-text",
                    "--set-image",
                    "--set-image-coords",
                    "--set-image-slot",
                    "--image-fit",
                    "--insert-after",
                    "--level",
                    "--align",
                    "--bullet-mode",
                    "--bullet-char",
                    "--auto-num",
                    "--space-before",
                    "--space-after",
                    "--line-spacing",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_new_slide_from_layout(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "place" && verb == "image" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--image",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--name",
                    "--fit-mode",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_place_image(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "place" && verb == "table" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--data",
                    "--format",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--name",
                    "--header-color",
                    "--band1-color",
                    "--band2-color",
                    "--font-size",
                    "--border-color",
                    "--border-width",
                    "--out",
                    "--backup",
                ],
                &[
                    "--header",
                    "--banded-rows",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_place_table(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "place" && verb == "table-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--table",
                    "--max-cells",
                    "--formula-mode",
                    "--expect-source-range",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--name",
                    "--header-color",
                    "--band1-color",
                    "--band2-color",
                    "--font-size",
                    "--border-color",
                    "--border-width",
                    "--out",
                    "--backup",
                ],
                &[
                    "--header",
                    "--banded-rows",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_place_table_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "animations" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_animations_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "animations" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--shape",
                    "--effect",
                    "--direction",
                    "--duration-ms",
                    "--start",
                    "--paragraph-range",
                    "--expect-shape-name",
                    "--expect-paragraph-count",
                    "--out",
                    "--backup",
                ],
                &["--by-paragraph", "--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_animations_add(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "animations" && verb == "remove" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--effect-id",
                    "--expect-shape-name",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_animations_remove(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "animations" && verb == "reorder" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--order", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_animations_reorder(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "animations" && verb == "prune-stale" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_animations_prune_stale(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "selectors" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_slide_selectors(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &["--include-text", "--include-bounds"])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_show(file, slide, include_text, include_bounds)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "get" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target"],
                &["--include-text", "--include-bounds"],
            )?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let target = parse_string_flag(rest, "--target")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"target\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_get(file, slide, &target, include_text, include_bounds)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "set-bounds" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target", "--bounds", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_shapes_set_bounds(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_shapes_delete(file, rest)
        }
        [family, group, verb, file] if family == "pptx" && group == "slides" && verb == "list" => {
            pptx_slides_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "import-slide" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--source",
                    "--slide",
                    "--insert-after",
                    "--layout-policy",
                    "--theme-policy",
                    "--notes-policy",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_slides_import_slide(file, rest)
        }
        [family, group, verb, file, source, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "merge" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout-policy", "--theme-policy", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_slides_merge(file, source, rest)
        }
        [family, group, verb, file, slide, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let slide = parse_pptx_slide_lifecycle_position(slide, "slide number")?;
            pptx_slides_delete(file, slide, rest)
        }
        [
            family,
            group,
            verb,
            file,
            from_position,
            to_position,
            rest @ ..,
        ] if family == "pptx" && group == "slides" && verb == "move" => {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let from_position =
                parse_pptx_slide_lifecycle_position(from_position, "from-position")?;
            let to_position = parse_pptx_slide_lifecycle_position(to_position, "to-position")?;
            pptx_slides_move(file, from_position, to_position, rest)
        }
        [family, group, verb, file, order, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "reorder" =>
        {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_slides_reorder(file, order, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "notes" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_notes(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "images" =>
        {
            reject_unknown_flags(rest, &["--out", "--slide"], &["--include-layout-images"])?;
            pptx_extract_images(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "xml" =>
        {
            reject_unknown_flags(rest, &["--slide", "--layout", "--master", "--out"], &[])?;
            pptx_extract_xml(file, rest)
        }
        [family, group, verb]
            if family == "pptx"
                && group == "media"
                && matches!(verb.as_str(), "list" | "add" | "replace") =>
        {
            Err(CliError::invalid_args("accepts 1 arg(s), received 0"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "media" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            pptx_media_list(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "media" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--file",
                    "--kind",
                    "--poster",
                    "--name",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--play-trigger",
                    "--volume",
                    "--insert-after-shape",
                    "--out",
                    "--backup",
                ],
                &[
                    "--play-cmd",
                    "--mute",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_media_add(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "media" && verb == "replace" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--shape",
                    "--shape-name",
                    "--file",
                    "--kind",
                    "--poster",
                    "--volume",
                    "--expect-shape-name",
                    "--expect-media-kind",
                    "--out",
                    "--backup",
                ],
                &["--mute", "--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_media_replace(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_notes_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--text", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_notes_set(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_notes_clear(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide", "--comment-id"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            if let Some(slide) = slide
                && slide < 1
            {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            if comment_id.is_some() && slide.is_none() {
                return Err(CliError::invalid_args("--comment-id requires --slide"));
            }
            pptx_comments_list(file, slide.map(|value| value as u32), comment_id)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--author",
                    "--initials",
                    "--date",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_add(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "edit" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--comment-id",
                    "--author-id",
                    "--handle",
                    "--text",
                    "--text-file",
                    "--author",
                    "--date",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_edit(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "remove" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--comment-id",
                    "--author-id",
                    "--handle",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_remove(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "fields" && verb == "inspect" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_fields_inspect(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "fields" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &["--footer", "--date-format", "--out", "--backup"],
                &[
                    "--show-footer",
                    "--show-slide-number",
                    "--show-date",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_fields_set(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "theme" && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--color",
                    "--major-font",
                    "--minor-font",
                    "--mode",
                    "--slide",
                    "--for-slides",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_theme_update(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_masters_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?.unwrap_or(1);
            pptx_masters_show(file, master)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "add-placeholder" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--master", "--type", "--bounds", "--idx", "--size", "--orient", "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_masters_add_placeholder(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "import" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--source",
                    "--master",
                    "--theme-policy",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_masters_import(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?;
            if let Some(master) = master
                && master < 0
            {
                return Err(CliError::invalid_args("--master must be >= 0"));
            }
            pptx_layouts_list(file, master.map(|value| value as u32))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--layout"], &[])?;
            let layout = parse_string_flag(rest, "--layout")?
                .ok_or_else(|| CliError::invalid_args("--layout flag is required"))?;
            pptx_layouts_show(file, &layout)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "clone" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--name", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_clone(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "import" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--source",
                    "--layout",
                    "--theme-policy",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_import(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "rename" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--name", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_rename(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "set-bounds" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--target", "--bounds", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_set_bounds(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "delete-shape" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--target", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_delete_shape(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "add-placeholder" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--layout", "--type", "--bounds", "--idx", "--size", "--orient", "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_add_placeholder(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            pptx_charts_list(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--chart"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            let chart = parse_string_flag(rest, "--chart")?;
            pptx_charts_show(file, slide, chart.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "create" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--type",
                    "--title",
                    "--values-json",
                    "--values-file",
                    "--source-file",
                    "--source-sheet",
                    "--source-range",
                    "--expect-source-range",
                    "--max-cells",
                    "--x",
                    "--y",
                    "--cx",
                    "--cy",
                    "--out",
                    "--backup",
                ],
                &[
                    "--embed-workbook",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_charts_create(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "update-data" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--series",
                    "--values",
                    "--values-json",
                    "--categories",
                    "--categories-json",
                    "--expect-point-count",
                    "--expect-values-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_update_data(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-title" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--title",
                    "--expect-title",
                    "--font-family",
                    "--font-size",
                    "--font-color",
                    "--out",
                    "--backup",
                ],
                &[
                    "--font-bold",
                    "--font-italic",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_charts_set_title(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-legend" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--position",
                    "--overlay",
                    "--expect-position",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_legend(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-chart-area-fill" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--fill-color",
                    "--expect-fill",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_chart_area_fill(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-plot-area-fill" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--fill-color",
                    "--expect-fill",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_plot_area_fill(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-series-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--series",
                    "--fill-color",
                    "--line-color",
                    "--line-width-pt",
                    "--marker-symbol",
                    "--marker-size",
                    "--expect-series-count",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_series_style(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "convert-type" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--to",
                    "--expect-type",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_convert_type(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-axis" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--axis",
                    "--title",
                    "--expect-axis-title",
                    "--hidden",
                    "--min",
                    "--max",
                    "--major-unit",
                    "--number-format",
                    "--major-gridlines",
                    "--minor-gridlines",
                    "--tick-label-font-family",
                    "--tick-label-font-size",
                    "--tick-label-font-color",
                    "--title-font-family",
                    "--title-font-size",
                    "--title-font-color",
                    "--expect-axis-count",
                    "--out",
                    "--backup",
                ],
                &[
                    "--tick-label-font-bold",
                    "--tick-label-font-italic",
                    "--title-font-bold",
                    "--title-font-italic",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_charts_set_axis(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "copy-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--to-chart",
                    "--from",
                    "--from-slide",
                    "--from-chart",
                    "--expect-series-count",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_copy_style(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--table-id", "--target"], &["--details"])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            if slide < 1 {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            let table_id = parse_i64_flag(rest, "--table-id")?.unwrap_or(0);
            if table_id < 0 {
                return Err(CliError::invalid_args(
                    "--table-id must be a positive integer",
                ));
            }
            let target = parse_string_flag(rest, "--target")?;
            if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                return Err(CliError::invalid_args(
                    "specify only one of --target or --table-id",
                ));
            }
            pptx_tables_show(
                file,
                slide as u32,
                table_id as u32,
                target.as_deref(),
                has_flag(rest, "--details"),
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "delete-row" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--row",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_delete_row(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "insert-row" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--at",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_insert_row(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "delete-col" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--col",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_delete_col(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "insert-col" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--at",
                    "--width-emu",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_insert_col(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "set-cell" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--row",
                    "--col",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_set_cell(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "update-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--table",
                    "--max-cells",
                    "--formula-mode",
                    "--expect-source-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_update_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "text" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--target",
                    "--paragraph",
                    "--run-index",
                    "--underline",
                    "--font-size",
                    "--color",
                    "--font-family",
                    "--hyperlink",
                    "--out",
                    "--backup",
                ],
                &[
                    "--bold",
                    "--italic",
                    "--remove-bold",
                    "--remove-italic",
                    "--remove-underline",
                    "--remove-font-size",
                    "--remove-color",
                    "--remove-font-family",
                    "--remove-hyperlink",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_text_set(file, rest)
        }
        [family, group, verb] if family == "pptx" && group == "translate" && verb == "export" => {
            Err(CliError::invalid_args("accepts 1 arg(s), received 0"))
        }
        [family, group, verb] if family == "pptx" && group == "translate" && verb == "apply" => {
            Err(CliError::invalid_args("accepts 2 arg(s), received 0"))
        }
        [family, group, verb, _file]
            if family == "pptx" && group == "translate" && verb == "apply" =>
        {
            Err(CliError::invalid_args("accepts 2 arg(s), received 1"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "translate" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--source-lang", "--target-lang", "--format"],
                &["--include-notes"],
            )?;
            pptx_translate_export(file, rest)
        }
        [family, group, verb, file, manifest, rest @ ..]
            if family == "pptx" && group == "translate" && verb == "apply" =>
        {
            reject_unknown_flags(rest, &["--stale", "--output"], &[])?;
            pptx_translate_apply(file, manifest, rest)
        }
        [family, group, verb]
            if family == "pptx"
                && group == "replace"
                && (verb.as_str() == "text-from-xlsx" || verb.as_str() == "text-map-from-xlsx") =>
        {
            Err(CliError::invalid_args("accepts 1 arg(s), received 0"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide", "--target", "--text", "--out"], &[])?;
            pptx_replace_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--target",
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--max-cells",
                    "--formula-mode",
                    "--mode",
                    "--row-sep",
                    "--col-sep",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_text_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-map-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--table",
                    "--max-cells",
                    "--formula-mode",
                    "--mode",
                    "--slide-col",
                    "--target-col",
                    "--text-col",
                    "--expect-source-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_text_map_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-occurrences" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--match-text",
                    "--new-text",
                    "--new-text-file",
                    "--for-slides",
                    "--for-shape",
                    "--expect-count",
                    "--expect-plan-hash",
                    "--out",
                    "--backup",
                ],
                &[
                    "--ignore-case",
                    "--allow-zero",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_replace_text_occurrences(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "images" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--target",
                    "--image",
                    "--fit-mode",
                    "--slide",
                    "--for-slides",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_images(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn convert_xlsm_to_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &["--out"],
        &["--dry-run", "--no-validate", "--in-place"],
    )?;
    if !path_has_extension(file, ".xlsm") {
        return Err(CliError::invalid_args(
            "convert xlsm-to-xlsx expects an .xlsm input; use `ooxml --json convert xlsm-to-xlsx <input.xlsm> --out <output.xlsx>`",
        ));
    }
    if has_flag(args, "--in-place") {
        return Err(CliError::invalid_args(
            "convert xlsm-to-xlsx writes a non-macro .xlsx save-as output; use --out <output.xlsx>, or use `ooxml --json vba remove <file> --in-place` for generic macro removal",
        ));
    }

    let dry_run = has_flag(args, "--dry-run");
    let out = parse_string_flag(args, "--out")?;
    if !dry_run {
        let Some(out) = out.as_deref() else {
            return Err(CliError::invalid_args(
                "convert xlsm-to-xlsx requires --out <output.xlsx>",
            ));
        };
        if !path_has_extension(out, ".xlsx") {
            return Err(CliError::invalid_args(
                "convert xlsm-to-xlsx requires an .xlsx --out path",
            ));
        }
    }

    let value = vba_remove(
        file,
        VbaMutationOptions {
            out: out.as_deref(),
            backup: None,
            dry_run,
            no_validate: has_flag(args, "--no-validate"),
            in_place: false,
        },
    )?;
    Ok(add_xlsm_to_xlsx_conversion_metadata(
        value,
        file,
        out.as_deref(),
        dry_run,
    ))
}

fn positional_args<'a>(
    args: &'a [String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> CliResult<Vec<&'a str>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if !arg.starts_with("--") {
            out.push(arg.as_str());
            index += 1;
            continue;
        }
        if let Some((flag, _)) = arg.split_once('=') {
            if value_flags.iter().any(|known| known == &flag)
                || bool_flags.iter().any(|known| known == &flag)
            {
                index += 1;
                continue;
            }
        }
        if bool_flags.iter().any(|flag| flag == arg) {
            index += 1;
            continue;
        }
        if value_flags.iter().any(|flag| flag == arg) {
            if args.get(index + 1).is_none() {
                return Err(CliError::invalid_args(format!("{arg} requires a value")));
            }
            index += 2;
            continue;
        }
        return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
    }
    Ok(out)
}

fn add_xlsm_to_xlsx_conversion_metadata(
    mut value: Value,
    input: &str,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let Value::Object(object) = &mut value else {
        return value;
    };
    let validate_command_key = if dry_run {
        "validateCommandTemplate"
    } else {
        "validateCommand"
    };
    let conformance_command_key = if dry_run {
        "conformanceCommandTemplate"
    } else {
        "conformanceCommand"
    };
    let validate_command = object.get(validate_command_key).cloned();
    let conformance_command = object.get(conformance_command_key).cloned();
    let mut conversion = json!({
        "alias": "xlsm-to-xlsx",
        "implementation": "vba remove",
        "input": input,
        "sourceExtension": ".xlsm",
        "targetExtension": ".xlsx",
        "macroRemovalCommand": format!(
            "ooxml --json vba remove {} --out {}",
            command_arg(input),
            command_arg(output.unwrap_or("<out.xlsx>"))
        ),
        "changed": [
            "main workbook content type changed from macro-enabled to non-macro xlsx"
        ],
        "removed": [
            "vbaProject.bin part",
            "VBA project relationships",
            "VBA project content type override"
        ],
        "dryRun": dry_run,
    });
    if let Some(output) = output.filter(|_| !dry_run)
        && let Value::Object(conversion_object) = &mut conversion
    {
        conversion_object.insert("output".to_string(), json!(output));
    }
    if let Some(validate_command) = validate_command
        && let Value::Object(conversion_object) = &mut conversion
    {
        conversion_object.insert(validate_command_key.to_string(), validate_command);
    }
    if let Some(conformance_command) = conformance_command
        && let Value::Object(conversion_object) = &mut conversion
    {
        conversion_object.insert(
            conformance_command_key.to_string(),
            conformance_command.clone(),
        );
        conversion_object.insert("proofCommand".to_string(), conformance_command);
    }
    object.insert("conversion".to_string(), conversion);
    value
}

fn path_has_extension(path: &str, extension: &str) -> bool {
    path.to_ascii_lowercase()
        .ends_with(&extension.to_ascii_lowercase())
}

fn parse_pptx_slide_lifecycle_position(value: &str, label: &str) -> CliResult<i64> {
    value.parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!("invalid {label}: {value} (expected an integer)"))
    })
}

pub(crate) fn require_docx_block_hash(value: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Err(CliError::invalid_args("--expect-hash is required"));
    }
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    }
    Ok(())
}
