# OpenXML Validator — the Linux repair-prompt oracle

LibreOffice and `ooxml conformance check` are **lenient**: they open/validate files
that desktop Microsoft Office rejects with "needs repair." This tool runs
**Microsoft's own Open XML SDK schema validator** (`OpenXmlValidator`, targeting the
Office2019 schema) on Linux, so you can catch repair-triggering violations
(out-of-order elements, bad enum values, illegal children, etc.) in your dev loop —
without a Windows/Office machine.

## Why it exists

A file with child elements out of schema order (e.g. `spPr` before `nvSpPr` in a
`p:sp`) renders fine in LibreOffice and passes `ooxml conformance check`, but makes
desktop PowerPoint show the repair dialog. The Open XML SDK validator catches it and
reports the exact part + XPath.

## Setup (one time)

```bash
# install .NET 8 SDK (no root needed)
curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0 --install-dir "$HOME/dotnet"
export PATH="$PATH:$HOME/dotnet"
```

## Build & run

```bash
cd tools/openxml-validator
dotnet build -c Release
dotnet run -c Release --no-build -- /path/to/file.pptx   # also .docx / .xlsx
```

Exit code `0` = clean, `1` = schema errors (printed with part URI + XPath),
`2` = usage error. Suitable for CI / pre-commit gating.

## Oracle tiers (weakest → strongest)

1. `ooxml validate` / `conformance check` — your fast invariants. Lenient.
2. LibreOffice render — proves it opens *somewhere*. Lenient.
3. **This validator** — Microsoft's real schema. Strongest automated Linux signal.
4. `tools/windows-office-oracle.ps1` on a Windows+Office box — ground-truth repair check.

## Suggested follow-up

`conformance check` missed the element-ordering violation this tool caught. Worth
either hardening `repair-invariants` to assert CT_Shape/CT_* child sequences, or
shelling out to this validator from the conformance harness when .NET is present.
