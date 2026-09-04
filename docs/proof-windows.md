# Windows release proof on Legion

Run the complete proof from an interactive PowerShell session at the repository
root:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File .\tools\legion-proof.ps1
```

That is the whole operator command. The script checks Cargo, a real .NET SDK,
the validator source, and the Excel, PowerPoint, and Word COM registrations
before it starts expensive work. A failed prerequisite includes the command or
installation step to correct it. Office must have completed its first-run
screens in the same logged-in desktop session; session 0 and non-interactive SSH
sessions are not valid Office proof environments.

The runner then:

1. builds the release `ooxml.exe`;
2. builds the Microsoft Open XML SDK validator;
3. runs the pinned 152-command mutation-envelope contract and retains its four
   family evidence files;
4. runs `windows-office-edit-smoke.ps1` with conformance, required SDK
   validation, the artifact-proof matrix, and Office COM enabled; and
5. builds the canonical PPTX, XLSX, DOCX, Markdown-to-PPTX, and
   Markdown-to-DOCX recipes, validates each one, and opens and saves each one
   through its desktop Office application.

## Results

The command exits zero only when every required stage and all five recipes pass.
It writes the complete evidence tree beneath `target/legion-proof/`. Start with:

- `target/legion-proof/summary.json` for machine-readable automation;
- `target/legion-proof/report.md` for the operator report;
- `target/legion-proof/office-edit-smoke/summary.json` for every contract-smoke
  scenario; and
- `target/legion-proof/office-roundtrips/` for the Office-saved recipe copies.

The Markdown report has separate prerequisite, stage, scenario, and recipe
tables. Its recipe table records build, strict validation, Open XML SDK, Office
open/save, repair-prompt assessment, and both the generated-input and
Office-saved SHA-256 hashes. Hashes are evidence of the exact two artifacts;
they are not expected to match because Office normally rewrites package bytes
when it saves. `sourceUnchanged: true` in the JSON proves that the original
generated recipe was not overwritten.

A successful bounded COM open/save records `repairPromptDetected: false`. A
timeout records it as `true` and fails the proof because a modal repair,
recovery, first-run, or add-in prompt may be blocking Office. Other COM failures
record the value as `null` rather than pretending that prompt state was known.
If diagnosis needs visible Office windows, rerun with `-Visible`.

## Non-Office rehearsal

Maintainers can exercise the same build, contract, strict, conformance, SDK,
recipe, summary, and report path without desktop Office:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File ./tools/legion-proof.ps1 -SkipOffice
```

`-SkipOffice` is intentionally labeled as non-Office proof in both outputs. It
can pass on Linux under PowerShell 7, but it does not establish Microsoft Office
compatibility. The normal Legion command above is the required desktop Office
run.
