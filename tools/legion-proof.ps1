[CmdletBinding()]
param(
    [string]$RepoRoot = "",
    [string]$OutputDir = "",
    [string]$CargoExe = "",
    [string]$DotNetExe = "",
    [string]$BinaryPath = "",
    [int]$MutationParallelism = ([Math]::Max(1, [Math]::Min(4, [Environment]::ProcessorCount))),
    [int]$OfficeTimeoutSeconds = 180,
    [switch]$SkipOffice,
    [switch]$Visible,
    [switch]$RenderReportOnly,
    [string]$SummaryFixture = "",
    [string]$ReportPath = "",
    [switch]$OfficeRoundTripChild,
    [string]$ChildInput = "",
    [string]$ChildOutput = "",
    [string]$ChildFamily = "",
    [string]$ChildResultJson = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$script:IsWindowsPlatform = $env:OS -eq "Windows_NT"

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Content
    )

    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function ConvertTo-OneLine {
    param([object]$Value)

    if ($null -eq $Value) {
        return ""
    }
    return [regex]::Replace(([string]$Value).Trim(), '\s+', ' ')
}

function Escape-MarkdownCell {
    param([object]$Value)

    $text = ConvertTo-OneLine -Value $Value
    if ($text -eq "") {
        return "-"
    }
    return $text.Replace("|", "\|")
}

function Get-PropertyValue {
    param(
        [object]$Object,
        [string]$Name,
        [object]$Default = $null
    )

    if ($null -eq $Object) {
        return $Default
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property -or $null -eq $property.Value) {
        return $Default
    }
    return $property.Value
}

function Get-LegionScenarioStatus {
    param(
        [object]$Scenario,
        [bool]$OfficeWasSkipped
    )

    foreach ($stageName in @("mutation", "readback", "validation", "conformance", "openXmlSdk")) {
        $stage = Get-PropertyValue $Scenario $stageName $null
        if ([string](Get-PropertyValue $stage "status" "missing") -ne "passed") {
            return "failed"
        }
    }
    $officeStatus = [string](Get-PropertyValue (Get-PropertyValue $Scenario "microsoftOffice" $null) "status" "missing")
    if ($OfficeWasSkipped) {
        if ($officeStatus -eq "skipped") { return "passed" }
        return "failed"
    }
    if ($officeStatus -eq "passed") { return "passed" }
    return "failed"
}

function Write-LegionMarkdownReport {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Summary,
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# ooxml-cli Legion proof report")
    $lines.Add("")
    $lines.Add(("- Overall status: **{0}**" -f (Escape-MarkdownCell (Get-PropertyValue $Summary "status" "unknown"))))
    $generatedAt = Get-PropertyValue $Summary "generatedAtUtc" "unknown"
    if ($generatedAt -is [DateTime]) {
        $generatedAt = $generatedAt.ToUniversalTime().ToString("o")
    }
    $lines.Add(('- Generated: `{0}`' -f (Escape-MarkdownCell $generatedAt)))
    $lines.Add(('- Host: `{0}`' -f (Escape-MarkdownCell (Get-PropertyValue $Summary "host" "unknown"))))
    $lines.Add(('- Office proof requested: `{0}`' -f (-not [bool](Get-PropertyValue $Summary "skipOffice" $false))))
    $lines.Add("")
    $lines.Add('A `passed` Office row means the generated package opened and was saved by the matching desktop Office COM application inside a bounded child process. A `skipped` row is not Office proof. A timeout is a failure and is recorded as a suspected modal repair or recovery prompt.')
    $lines.Add("")
    $lines.Add("## Prerequisites")
    $lines.Add("")
    $lines.Add("| Check | Status | Detail |")
    $lines.Add("|---|---|---|")
    foreach ($row in @(Get-PropertyValue $Summary "prerequisites" @())) {
        $lines.Add(("| {0} | {1} | {2} |" -f (Escape-MarkdownCell (Get-PropertyValue $row "id" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "status" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "detail" ""))))
    }
    $lines.Add("")
    $lines.Add("## Stages")
    $lines.Add("")
    $lines.Add("| Stage | Status | Detail | Artifact |")
    $lines.Add("|---|---|---|---|")
    foreach ($row in @(Get-PropertyValue $Summary "stages" @())) {
        $lines.Add(("| {0} | {1} | {2} | {3} |" -f (Escape-MarkdownCell (Get-PropertyValue $row "id" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "status" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "detail" "")), (Escape-MarkdownCell (Get-PropertyValue $row "artifact" ""))))
    }
    $lines.Add("")
    $lines.Add("## Contract scenarios")
    $lines.Add("")
    $lines.Add("| Scenario | Family | Status | Proof level | Output |")
    $lines.Add("|---|---|---|---|---|")
    $reportOfficeWasSkipped = [bool](Get-PropertyValue $Summary "skipOffice" $false)
    foreach ($row in @(Get-PropertyValue $Summary "scenarios" @())) {
        $scenarioStatus = Get-LegionScenarioStatus $row $reportOfficeWasSkipped
        $lines.Add(("| {0} | {1} | {2} | {3} | {4} |" -f (Escape-MarkdownCell (Get-PropertyValue $row "name" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "family" "unknown")), (Escape-MarkdownCell $scenarioStatus), (Escape-MarkdownCell (Get-PropertyValue $row "proofLevel" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "output" ""))))
    }
    $lines.Add("")
    $lines.Add("## Canonical recipes")
    $lines.Add("")
    $lines.Add("| Recipe | Family | Build | Strict | SDK | Office open/save | Repair prompt | Input SHA-256 | Saved SHA-256 |")
    $lines.Add("|---|---|---|---|---|---|---|---|---|")
    foreach ($row in @(Get-PropertyValue $Summary "recipes" @())) {
        $office = Get-PropertyValue $row "office" $null
        $hashes = Get-PropertyValue $row "roundTrip" $null
        $repairValue = Get-PropertyValue $office "repairPromptDetected" $null
        $repairText = "unknown"
        if ($repairValue -eq $true) { $repairText = "suspected" }
        if ($repairValue -eq $false) { $repairText = "not detected" }
        $lines.Add(('| {0} | {1} | {2} | {3} | {4} | {5} | {6} | `{7}` | `{8}` |' -f (Escape-MarkdownCell (Get-PropertyValue $row "id" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $row "family" "unknown")), (Escape-MarkdownCell (Get-PropertyValue (Get-PropertyValue $row "build" $null) "status" "unknown")), (Escape-MarkdownCell (Get-PropertyValue (Get-PropertyValue $row "strictValidation" $null) "status" "unknown")), (Escape-MarkdownCell (Get-PropertyValue (Get-PropertyValue $row "openXmlSdk" $null) "status" "unknown")), (Escape-MarkdownCell (Get-PropertyValue $office "status" "unknown")), (Escape-MarkdownCell $repairText), (Escape-MarkdownCell (Get-PropertyValue $hashes "inputSha256" "")), (Escape-MarkdownCell (Get-PropertyValue $hashes "savedSha256" ""))))
    }
    $lines.Add("")
    $counts = Get-PropertyValue $Summary "counts" $null
    $lines.Add("## Totals")
    $lines.Add("")
    $lines.Add(("- Scenarios: {0}/{1} passed." -f (Get-PropertyValue $counts "scenariosPassed" 0), (Get-PropertyValue $counts "scenariosTotal" 0)))
    $lines.Add(("- Recipes: {0}/{1} passed." -f (Get-PropertyValue $counts "recipesPassed" 0), (Get-PropertyValue $counts "recipesTotal" 0)))
    $lines.Add(("- Failed required stages: {0}." -f (Get-PropertyValue $counts "failedRequiredStages" 0)))
    $lines.Add("")

    $parent = Split-Path -Parent $Path
    if ($parent -ne "") {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    Write-Utf8NoBom -Path $Path -Content (($lines -join [Environment]::NewLine) + [Environment]::NewLine)
}

function ConvertTo-NativeArgument {
    param([string]$Value)

    if ($Value -eq "") {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    return '"' + ($Value -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1') + '"'
}

function Format-CommandLine {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    return ((@($FilePath) + @($Arguments)) | ForEach-Object { ConvertTo-NativeArgument ([string]$_) }) -join " "
}

function Invoke-NativeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = "",
        [hashtable]$Environment = @{}
    )

    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $FilePath
    $start.Arguments = (@($Arguments) | ForEach-Object { ConvertTo-NativeArgument ([string]$_) }) -join " "
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    if ($WorkingDirectory -ne "") {
        $start.WorkingDirectory = $WorkingDirectory
    }
    foreach ($key in $Environment.Keys) {
        $start.EnvironmentVariables[[string]$key] = [string]$Environment[$key]
    }

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $timer.Stop()
    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result
    $exitCode = $process.ExitCode
    $process.Dispose()

    [pscustomobject]@{
        command   = Format-CommandLine -FilePath $FilePath -Arguments $Arguments
        exitCode  = $exitCode
        stdout    = $stdout
        stderr    = $stderr
        elapsedMs = $timer.ElapsedMilliseconds
    }
}

function New-Result {
    param(
        [string]$Id,
        [string]$Status,
        [string]$Detail,
        [string]$Command = "",
        [string]$Artifact = "",
        [int64]$ElapsedMs = 0
    )

    [pscustomobject]@{
        id        = $Id
        status    = $Status
        detail    = $Detail
        command   = $Command
        artifact  = $Artifact
        elapsedMs = $ElapsedMs
    }
}

function Get-CommandPath {
    param(
        [string]$Requested,
        [string]$Name
    )

    if ($Requested -ne "") {
        if (Test-Path -LiteralPath $Requested -PathType Leaf) {
            return (Resolve-Path -LiteralPath $Requested).Path
        }
        $requestedCommand = Get-Command $Requested -ErrorAction SilentlyContinue
        if ($null -ne $requestedCommand) {
            return $requestedCommand.Source
        }
        return ""
    }
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        return ""
    }
    return $command.Source
}

function Resolve-DotNetPath {
    param([string]$Requested)

    if ($Requested -ne "") {
        return Get-CommandPath -Requested $Requested -Name "dotnet"
    }
    $userProfile = [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
    $privateDotNet = Join-Path (Join-Path $userProfile "dotnet") $(if ($script:IsWindowsPlatform) { "dotnet.exe" } else { "dotnet" })
    if (Test-Path -LiteralPath $privateDotNet -PathType Leaf) {
        return $privateDotNet
    }
    return Get-CommandPath -Requested "" -Name "dotnet"
}

function Release-ComObject {
    param([object]$Object)

    if ($null -ne $Object -and [System.Runtime.InteropServices.Marshal]::IsComObject($Object)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Object)
    }
}

function Set-AutomationSecurity {
    param([object]$Application)

    try { $Application.AutomationSecurity = 3 } catch {}
}

function Get-OfficeIdentity {
    param([object]$Application)

    $version = ""
    $build = ""
    try { $version = [string]$Application.Version } catch {}
    try { $build = [string]$Application.Build } catch {}
    return [pscustomobject]@{ version = $version; build = $build }
}

function Invoke-OfficeRoundTripChild {
    param(
        [string]$InputPath,
        [string]$OutputPath,
        [string]$Family
    )

    $application = $null
    $document = $null
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $officeName = switch ($Family) { "xlsx" { "Excel" } "pptx" { "PowerPoint" } "docx" { "Word" } default { "unknown" } }
    $identity = [pscustomobject]@{ version = ""; build = "" }
    $inputHash = (Get-FileHash -LiteralPath $InputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $status = "failed"
    $openStatus = "failed"
    $saveStatus = "not-run"
    $errorType = ""
    $errorMessage = ""

    try {
        if (Test-Path -LiteralPath $OutputPath) {
            Remove-Item -LiteralPath $OutputPath -Force
        }
        switch ($Family) {
            "xlsx" {
                $application = New-Object -ComObject Excel.Application
                $application.Visible = [bool]$Visible
                $application.DisplayAlerts = $false
                Set-AutomationSecurity $application
                $identity = Get-OfficeIdentity $application
                $document = $application.Workbooks.Open($InputPath, 0, $false)
                $openStatus = "passed"
                $document.SaveAs($OutputPath, 51)
            }
            "pptx" {
                $application = New-Object -ComObject PowerPoint.Application
                try { $application.DisplayAlerts = 1 } catch {}
                Set-AutomationSecurity $application
                $identity = Get-OfficeIdentity $application
                $document = $application.Presentations.Open($InputPath, $false, $false, [bool]$Visible)
                $openStatus = "passed"
                $document.SaveAs($OutputPath, 24)
            }
            "docx" {
                $application = New-Object -ComObject Word.Application
                $application.Visible = [bool]$Visible
                $application.DisplayAlerts = 0
                Set-AutomationSecurity $application
                $identity = Get-OfficeIdentity $application
                $document = $application.Documents.Open($InputPath, $false, $false)
                $openStatus = "passed"
                $document.SaveAs2($OutputPath, 12)
            }
            default { throw "Unsupported Office family: $Family" }
        }
        $saveStatus = "passed"
        if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
            throw "Office returned from SaveAs without writing $OutputPath"
        }
        $status = "passed"
    }
    catch {
        $errorType = $_.Exception.GetType().FullName
        $errorMessage = $_.Exception.Message
        if ($openStatus -eq "passed") {
            $saveStatus = "failed"
        }
    }
    finally {
        if ($null -ne $document) {
            try { $document.Close($false) } catch {}
        }
        if ($null -ne $application) {
            try { $application.Quit() } catch {}
        }
        Release-ComObject $document
        Release-ComObject $application
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        $timer.Stop()
    }

    $sourceHashAfter = ""
    $savedHash = ""
    if (Test-Path -LiteralPath $InputPath -PathType Leaf) {
        $sourceHashAfter = (Get-FileHash -LiteralPath $InputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    if (Test-Path -LiteralPath $OutputPath -PathType Leaf) {
        $savedHash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    return [pscustomobject]@{
        status                 = $status
        application            = $officeName
        officeVersion          = $identity.version
        officeBuild            = $identity.build
        openStatus             = $openStatus
        saveStatus             = $saveStatus
        repairPromptDetected   = if ($status -eq "passed") { $false } else { $null }
        repairPromptAssessment = if ($status -eq "passed") { "No modal repair or recovery prompt blocked the bounded open/save operation." } else { "Unknown; the COM operation failed without a timeout." }
        inputSha256            = $inputHash
        sourceSha256After      = $sourceHashAfter
        sourceUnchanged        = $inputHash -eq $sourceHashAfter
        savedSha256            = $savedHash
        elapsedMs              = $timer.ElapsedMilliseconds
        errorType              = $errorType
        errorMessage           = $errorMessage
    }
}

function Invoke-BoundedOfficeRoundTrip {
    param(
        [string]$ScriptPath,
        [string]$Root,
        [string]$InputPath,
        [string]$OutputPath,
        [string]$Family,
        [string]$ResultPath,
        [int]$TimeoutSeconds
    )

    $stdoutPath = $ResultPath + ".stdout.txt"
    $stderrPath = $ResultPath + ".stderr.txt"
    Remove-Item -LiteralPath $ResultPath, $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue
    $arguments = @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $ScriptPath,
        "-OfficeRoundTripChild", "-RepoRoot", $Root,
        "-ChildInput", $InputPath, "-ChildOutput", $OutputPath,
        "-ChildFamily", $Family, "-ChildResultJson", $ResultPath
    )
    if ($Visible) { $arguments += "-Visible" }
    $argumentLine = (@($arguments) | ForEach-Object { ConvertTo-NativeArgument ([string]$_) }) -join " "
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath "powershell.exe" -ArgumentList $argumentLine -WorkingDirectory $Root -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -WindowStyle Hidden -PassThru
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    $timer.Stop()
    if (-not $finished) {
        # The child PowerShell PID is created by this function and is therefore
        # the only process the parent may safely terminate. Office may have
        # launched an independent COM server; do not guess ownership by name or
        # start time and do not terminate it.
        try { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue } catch {}
        return [pscustomobject]@{
            status = "timeout"; application = switch ($Family) { "xlsx" { "Excel" } "pptx" { "PowerPoint" } "docx" { "Word" } }; officeVersion = ""; officeBuild = ""
            openStatus = "unknown"; saveStatus = "unknown"; repairPromptDetected = $true
            repairPromptAssessment = "The bounded COM operation timed out; a modal repair, recovery, first-run, or add-in prompt is suspected. The runner stopped only its own PowerShell child; close any visible Office prompt manually."
            inputSha256 = (Get-FileHash -LiteralPath $InputPath -Algorithm SHA256).Hash.ToLowerInvariant(); sourceSha256After = ""; sourceUnchanged = $null; savedSha256 = ""
            elapsedMs = $timer.ElapsedMilliseconds; errorType = "Timeout"; errorMessage = "Office open/save exceeded $TimeoutSeconds second(s)."
        }
    }
    if (Test-Path -LiteralPath $ResultPath -PathType Leaf) {
        return Get-Content -LiteralPath $ResultPath -Raw | ConvertFrom-Json
    }
    $detail = "Office child exited without writing a result."
    if (Test-Path -LiteralPath $stderrPath -PathType Leaf) {
        $stderr = (Get-Content -LiteralPath $stderrPath -Raw).Trim()
        if ($stderr -ne "") { $detail = $stderr }
    }
    return [pscustomobject]@{
        status = "failed"; application = switch ($Family) { "xlsx" { "Excel" } "pptx" { "PowerPoint" } "docx" { "Word" } }; officeVersion = ""; officeBuild = ""
        openStatus = "unknown"; saveStatus = "unknown"; repairPromptDetected = $null; repairPromptAssessment = "Unknown; the child process did not return a result."
        inputSha256 = (Get-FileHash -LiteralPath $InputPath -Algorithm SHA256).Hash.ToLowerInvariant(); sourceSha256After = ""; sourceUnchanged = $null; savedSha256 = ""
        elapsedMs = $timer.ElapsedMilliseconds; errorType = "ChildProcessFailed"; errorMessage = $detail
    }
}

if ($RenderReportOnly) {
    if ($SummaryFixture -eq "") { throw "-RenderReportOnly requires -SummaryFixture." }
    $fixturePath = (Resolve-Path -LiteralPath $SummaryFixture).Path
    if ($ReportPath -eq "") { $ReportPath = [System.IO.Path]::ChangeExtension($fixturePath, ".md") }
    $fixture = Get-Content -LiteralPath $fixturePath -Raw | ConvertFrom-Json
    Write-LegionMarkdownReport -Summary $fixture -Path $ReportPath
    Write-Output $ReportPath
    exit 0
}

if ($OfficeRoundTripChild) {
    if (-not $script:IsWindowsPlatform) { throw "Office COM round-trip children require Windows." }
    if ($ChildInput -eq "" -or $ChildOutput -eq "" -or $ChildFamily -eq "" -or $ChildResultJson -eq "") {
        throw "Office child mode requires -ChildInput, -ChildOutput, -ChildFamily, and -ChildResultJson."
    }
    $childResult = Invoke-OfficeRoundTripChild -InputPath $ChildInput -OutputPath $ChildOutput -Family $ChildFamily
    Write-Utf8NoBom -Path $ChildResultJson -Content (($childResult | ConvertTo-Json -Depth 8) + [Environment]::NewLine)
    if ($childResult.status -ne "passed") { exit 1 }
    exit 0
}

if ($RepoRoot -eq "") {
    $RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
}
$root = (Resolve-Path -LiteralPath $RepoRoot).Path
Set-Location -LiteralPath $root
if ($OutputDir -eq "") { $OutputDir = Join-Path (Join-Path $root "target") "legion-proof" }
$output = [System.IO.Path]::GetFullPath($OutputDir)
$recipesDir = Join-Path $output "recipes"
$roundTripDir = Join-Path $output "office-roundtrips"
$contractRoot = Join-Path $output "contract-proof"
$contractEvidenceDir = Join-Path $contractRoot "evidence"
$smokeDir = Join-Path $output "office-edit-smoke"
$summaryPath = Join-Path $output "summary.json"
if ($ReportPath -eq "") { $ReportPath = Join-Path $output "report.md" }
New-Item -ItemType Directory -Force -Path $output, $recipesDir, $roundTripDir, $contractRoot | Out-Null

$prerequisites = New-Object System.Collections.Generic.List[object]
$stages = New-Object System.Collections.Generic.List[object]
$recipes = New-Object System.Collections.Generic.List[object]
$scenarios = @()

$cargo = Get-CommandPath -Requested $CargoExe -Name "cargo"
if ($cargo -eq "") {
    $prerequisites.Add((New-Result "cargo" "failed" 'Cargo was not found. Install Rust with rustup, reopen PowerShell, and confirm `cargo --version`.'))
}
else {
    $cargoVersion = Invoke-NativeProcess $cargo @("--version") $root
    $prerequisites.Add((New-Result "cargo" $(if ($cargoVersion.exitCode -eq 0) { "passed" } else { "failed" }) $(if ($cargoVersion.exitCode -eq 0) { (ConvertTo-OneLine $cargoVersion.stdout) } else { 'Cargo did not run. Reinstall the pinned Rust toolchain and run `mise install`.' }) $cargoVersion.command "" $cargoVersion.elapsedMs))
}

$dotnet = Resolve-DotNetPath $DotNetExe
if ($dotnet -eq "") {
    $prerequisites.Add((New-Result "dotnet-sdk" "failed" '.NET SDK was not found. Install the .NET 8 SDK (a runtime alone is insufficient) and confirm `dotnet --list-sdks`.'))
}
else {
    $sdkList = Invoke-NativeProcess $dotnet @("--list-sdks") $root
    $sdkText = ConvertTo-OneLine $sdkList.stdout
    $sdkPassed = $sdkList.exitCode -eq 0 -and $sdkText -ne ""
    $prerequisites.Add((New-Result "dotnet-sdk" $(if ($sdkPassed) { "passed" } else { "failed" }) $(if ($sdkPassed) { $sdkText } else { ".NET was found, but no SDK is installed. Install the .NET 8 SDK; the runtime-only host cannot build the validator." }) $sdkList.command "" $sdkList.elapsedMs))
}

$validatorProject = Join-Path (Join-Path (Join-Path $root "tools") "openxml-validator") "openxml-validator.csproj"
if (Test-Path -LiteralPath $validatorProject -PathType Leaf) {
    $prerequisites.Add((New-Result "openxml-validator-source" "passed" "Validator project is present." "" $validatorProject))
}
else {
    $prerequisites.Add((New-Result "openxml-validator-source" "failed" "tools/openxml-validator/openxml-validator.csproj is missing; restore the repository checkout."))
}

$smokeScript = Join-Path (Join-Path $root "tools") "windows-office-edit-smoke.ps1"
if (Test-Path -LiteralPath $smokeScript -PathType Leaf) {
    $prerequisites.Add((New-Result "office-smoke-script" "passed" "Windows mutation and Office oracle smoke script is present." "" $smokeScript))
}
else {
    $prerequisites.Add((New-Result "office-smoke-script" "failed" "tools/windows-office-edit-smoke.ps1 is missing; restore the repository checkout."))
}

if ($SkipOffice) {
    $prerequisites.Add((New-Result "office-com" "skipped" "Skipped by -SkipOffice. This run proves the non-Office pipeline only and is not Microsoft Office compatibility evidence."))
}
elseif (-not $script:IsWindowsPlatform) {
    $prerequisites.Add((New-Result "office-com" "failed" "Desktop Office COM requires an interactive Windows session. Run with -SkipOffice for non-Office proof, or run this command on Legion."))
}
else {
    $missingProgIds = New-Object System.Collections.Generic.List[string]
    foreach ($progId in @("Excel.Application", "PowerPoint.Application", "Word.Application")) {
        if ($null -eq [type]::GetTypeFromProgID($progId)) { $missingProgIds.Add($progId) }
    }
    if ($missingProgIds.Count -eq 0) {
        $prerequisites.Add((New-Result "office-com" "passed" "Excel, PowerPoint, and Word COM registrations are present. The bounded recipe round trips provide the actual open/save proof."))
    }
    else {
        $prerequisites.Add((New-Result "office-com" "failed" ("Missing Office COM registrations: {0}. Install desktop Office and complete each application's first-run screens in this interactive user session." -f ($missingProgIds -join ", "))))
    }
}

$requiredPrerequisiteFailures = @($prerequisites | Where-Object { $_.status -eq "failed" })
$validatorDll = Join-Path (Join-Path (Join-Path (Join-Path (Split-Path -Parent $validatorProject) "bin") "Release") "net8.0") "openxml-validator.dll"

if ($requiredPrerequisiteFailures.Count -eq 0) {
    $targetRoot = $env:CARGO_TARGET_DIR
    if ([string]::IsNullOrWhiteSpace($targetRoot)) { $targetRoot = Join-Path $root "target" }
    if (-not [System.IO.Path]::IsPathRooted($targetRoot)) { $targetRoot = [System.IO.Path]::GetFullPath((Join-Path $root $targetRoot)) }
    if ($BinaryPath -eq "") {
        $binaryName = if ($script:IsWindowsPlatform) { "ooxml.exe" } else { "ooxml" }
        $BinaryPath = Join-Path (Join-Path $targetRoot "release") $binaryName
    }
    $BinaryPath = [System.IO.Path]::GetFullPath($BinaryPath)

    Write-Host "[1/5] Building the release ooxml binary..."
    $releaseBuild = Invoke-NativeProcess $cargo @("build", "--release", "--bin", "ooxml") $root @{ "CARGO_PROFILE_DEV_DEBUG" = "0" }
    $releasePassed = $releaseBuild.exitCode -eq 0 -and (Test-Path -LiteralPath $BinaryPath -PathType Leaf)
    $releaseDetail = if ($releasePassed) { "Release binary built successfully." } else { "Release build failed. " + (ConvertTo-OneLine $releaseBuild.stderr) }
    $stages.Add((New-Result "release-build" $(if ($releasePassed) { "passed" } else { "failed" }) $releaseDetail $releaseBuild.command $BinaryPath $releaseBuild.elapsedMs))

    Write-Host "[2/5] Building the Open XML SDK validator..."
    $validatorBuild = Invoke-NativeProcess $dotnet @("build", $validatorProject, "-c", "Release", "--nologo") $root @{ "DOTNET_CLI_TELEMETRY_OPTOUT" = "1"; "DOTNET_NOLOGO" = "1" }
    $validatorPassed = $validatorBuild.exitCode -eq 0 -and (Test-Path -LiteralPath $validatorDll -PathType Leaf)
    $validatorDetail = if ($validatorPassed) { "Open XML SDK validator built successfully." } else { "Validator build failed. " + (ConvertTo-OneLine $validatorBuild.stderr) }
    $stages.Add((New-Result "openxml-validator-build" $(if ($validatorPassed) { "passed" } else { "failed" }) $validatorDetail $validatorBuild.command $validatorDll $validatorBuild.elapsedMs))

    if ($releasePassed -and $validatorPassed) {
        Write-Host "[3/5] Producing the 152-command mutation contract evidence..."
        $contractTest = Invoke-NativeProcess $cargo @("test", "--test", "mutation_envelope", "mutation_commands_satisfy_the_envelope_contract", "--", "--nocapture") $root @{ "OOXML_CONTRACT_PROOF_DIR" = $contractRoot; "CARGO_PROFILE_DEV_DEBUG" = "0" }
        $evidenceFiles = @(Get-ChildItem -LiteralPath $contractEvidenceDir -Filter "*-contract-evidence.json" -File -ErrorAction SilentlyContinue)
        $contractPassed = $contractTest.exitCode -eq 0 -and $evidenceFiles.Count -eq 4
        $contractDetail = if ($contractPassed) { "Four family evidence files covering the pinned 152-command mutation matrix were written." } else { "Contract evidence failed or did not write exactly four family evidence files. " + (ConvertTo-OneLine $contractTest.stderr) }
        $stages.Add((New-Result "mutation-contract-evidence" $(if ($contractPassed) { "passed" } else { "failed" }) $contractDetail $contractTest.command $contractEvidenceDir $contractTest.elapsedMs))

        if ($contractPassed) {
            Write-Host "[4/5] Running strict, conformance, SDK, and Office scenario proof..."
            $hostExe = [System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
            $smokeArgs = @(
                "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $smokeScript,
                "-RepoRoot", $root, "-OutputDir", $smokeDir,
                "-BinaryPath", $BinaryPath, "-SkipBuild",
                "-DotNetExe", $dotnet, "-OpenXmlValidatorProject", $validatorProject,
                "-ContractEvidenceDir", $contractEvidenceDir,
                "-MutationParallelism", [string]$MutationParallelism,
                "-OfficeOracleTimeoutSeconds", [string]$OfficeTimeoutSeconds,
                "-RunConformance", "-RequireOpenXmlSdk",
                "-WriteArtifactProofMatrix", "-FailOnArtifactProofGap"
            )
            if ($SkipOffice) { $smokeArgs += "-SkipOffice" }
            if ($Visible) { $smokeArgs += "-Visible" }
            $smoke = Invoke-NativeProcess $hostExe $smokeArgs $root
            $smokeSummaryPath = Join-Path $smokeDir "summary.json"
            $smokeSummary = $null
            if (Test-Path -LiteralPath $smokeSummaryPath -PathType Leaf) {
                $smokeSummary = Get-Content -LiteralPath $smokeSummaryPath -Raw | ConvertFrom-Json
                $scenarios = @(Get-PropertyValue $smokeSummary "scenarios" @())
            }
            $smokePassed = $smoke.exitCode -eq 0 -and $null -ne $smokeSummary -and [string]$smokeSummary.status -eq "passed"
            $smokeDetail = if ($smokePassed) { "Windows edit smoke completed with every required proof tier passing." } else { "Windows edit smoke failed. " + (ConvertTo-OneLine $smoke.stderr) }
            $stages.Add((New-Result "windows-office-edit-smoke" $(if ($smokePassed) { "passed" } else { "failed" }) $smokeDetail $smoke.command $smokeSummaryPath $smoke.elapsedMs))
        }
        else {
            $stages.Add((New-Result "windows-office-edit-smoke" "skipped" "Contract evidence did not pass, so the dependent smoke proof was not run."))
        }

        Write-Host "[5/5] Building and validating the five canonical recipes..."
        $recipeDefinitions = @(
            [pscustomobject]@{ id = "deck-spec"; family = "pptx"; sourceKind = "spec"; source = "testdata/pptx/build-spec/q3-review.json"; file = "q3-review.pptx" },
            [pscustomobject]@{ id = "workbook-spec"; family = "xlsx"; sourceKind = "spec"; source = "testdata/xlsx/build-spec/sales.json"; file = "sales.xlsx" },
            [pscustomobject]@{ id = "document-spec"; family = "docx"; sourceKind = "spec"; source = "testdata/docx/build-spec/quarterly-report.json"; file = "quarterly-report.docx" },
            [pscustomobject]@{ id = "deck-markdown"; family = "pptx"; sourceKind = "markdown"; source = "testdata/markdown/q3-review.md"; file = "q3-review-markdown.pptx" },
            [pscustomobject]@{ id = "document-markdown"; family = "docx"; sourceKind = "markdown"; source = "testdata/markdown/quarterly-report.md"; file = "quarterly-report-markdown.docx" }
        )
        foreach ($definition in $recipeDefinitions) {
            $sourcePath = Join-Path $root $definition.source
            $recipeOutput = Join-Path $recipesDir $definition.file
            $sourceFlag = if ($definition.sourceKind -eq "spec") { "--spec" } else { "--from-markdown" }
            $build = Invoke-NativeProcess $BinaryPath @("--json", $definition.family, "build", $sourceFlag, $sourcePath, "--out", $recipeOutput, "--check", "--force") $root
            $buildPassed = $build.exitCode -eq 0 -and (Test-Path -LiteralPath $recipeOutput -PathType Leaf)
            $buildResult = New-Result "build" $(if ($buildPassed) { "passed" } else { "failed" }) $(if ($buildPassed) { "Canonical recipe built through the mutation seam." } else { ConvertTo-OneLine ($build.stderr + " " + $build.stdout) }) $build.command $recipeOutput $build.elapsedMs

            $strictResult = New-Result "strict-validation" "skipped" "Build did not produce a package."
            $sdkResult = New-Result "openxml-sdk" "skipped" "Build did not produce a package."
            if ($buildPassed) {
                $strict = Invoke-NativeProcess $BinaryPath @("--json", "validate", "--strict", $recipeOutput) $root
                $strictResult = New-Result "strict-validation" $(if ($strict.exitCode -eq 0) { "passed" } else { "failed" }) $(if ($strict.exitCode -eq 0) { "ooxml validate --strict accepted the package." } else { ConvertTo-OneLine ($strict.stderr + " " + $strict.stdout) }) $strict.command $recipeOutput $strict.elapsedMs
                $sdk = Invoke-NativeProcess $dotnet @($validatorDll, $recipeOutput) $root
                $sdkResult = New-Result "openxml-sdk" $(if ($sdk.exitCode -eq 0) { "passed" } else { "failed" }) $(if ($sdk.exitCode -eq 0) { "Open XML SDK reported zero errors." } else { ConvertTo-OneLine ($sdk.stderr + " " + $sdk.stdout) }) $sdk.command $recipeOutput $sdk.elapsedMs
            }

            $officeResult = [pscustomobject]@{
                status = "skipped"; application = switch ($definition.family) { "xlsx" { "Excel" } "pptx" { "PowerPoint" } "docx" { "Word" } }; officeVersion = ""; officeBuild = ""
                openStatus = "skipped"; saveStatus = "skipped"; repairPromptDetected = $null
                repairPromptAssessment = if ($SkipOffice) { "Skipped by -SkipOffice; no Office compatibility claim is made." } else { "Earlier package proof failed, so Office was not run." }
                errorType = ""; errorMessage = ""; elapsedMs = 0
            }
            $roundTrip = [pscustomobject]@{
                path = ""; inputSha256 = if ($buildPassed) { (Get-FileHash -LiteralPath $recipeOutput -Algorithm SHA256).Hash.ToLowerInvariant() } else { "" }
                sourceSha256After = ""; sourceUnchanged = $null; savedSha256 = ""; strictValidation = "not-run"; openXmlSdk = "not-run"
            }
            if (-not $SkipOffice -and $buildPassed -and $strictResult.status -eq "passed" -and $sdkResult.status -eq "passed") {
                $roundTripPath = Join-Path $roundTripDir $definition.file
                $childResultPath = Join-Path $roundTripDir ($definition.id + "-child.json")
                $officeResult = Invoke-BoundedOfficeRoundTrip -ScriptPath $PSCommandPath -Root $root -InputPath $recipeOutput -OutputPath $roundTripPath -Family $definition.family -ResultPath $childResultPath -TimeoutSeconds $OfficeTimeoutSeconds
                $roundTrip = [pscustomobject]@{
                    path = $roundTripPath; inputSha256 = $officeResult.inputSha256; sourceSha256After = $officeResult.sourceSha256After
                    sourceUnchanged = $officeResult.sourceUnchanged; savedSha256 = $officeResult.savedSha256; strictValidation = "not-run"; openXmlSdk = "not-run"
                }
                if ($officeResult.status -eq "passed") {
                    $roundStrict = Invoke-NativeProcess $BinaryPath @("--json", "validate", "--strict", $roundTripPath) $root
                    $roundSdk = Invoke-NativeProcess $dotnet @($validatorDll, $roundTripPath) $root
                    $roundTrip.strictValidation = if ($roundStrict.exitCode -eq 0) { "passed" } else { "failed" }
                    $roundTrip.openXmlSdk = if ($roundSdk.exitCode -eq 0) { "passed" } else { "failed" }
                    if ($roundStrict.exitCode -ne 0 -or $roundSdk.exitCode -ne 0 -or -not [bool]$roundTrip.sourceUnchanged) {
                        $officeResult.status = "failed"
                        $officeResult.errorType = "RoundTripValidationFailed"
                        $officeResult.errorMessage = "Office saved the package, but the saved copy failed strict/SDK validation or modified the source package."
                    }
                }
            }
            $recipes.Add([pscustomobject]@{
                id = $definition.id; family = $definition.family; sourceKind = $definition.sourceKind; source = $sourcePath; output = $recipeOutput
                build = $buildResult; strictValidation = $strictResult; openXmlSdk = $sdkResult; office = $officeResult; roundTrip = $roundTrip
            })
        }
        $canonicalFailures = @($recipes | Where-Object {
            $_.build.status -ne "passed" -or $_.strictValidation.status -ne "passed" -or $_.openXmlSdk.status -ne "passed" -or ((-not $SkipOffice) -and $_.office.status -ne "passed")
        })
        $canonicalStatus = if ($canonicalFailures.Count -eq 0 -and $recipes.Count -eq 5) { "passed" } else { "failed" }
        $canonicalDetail = if ($canonicalStatus -eq "passed") { "All five canonical recipes built and passed the required proof tiers." } else { ("{0} of 5 canonical recipes failed a required proof tier." -f $canonicalFailures.Count) }
        $stages.Add((New-Result "canonical-recipes" $canonicalStatus $canonicalDetail "" $recipesDir))
    }
    else {
        $stages.Add((New-Result "mutation-contract-evidence" "skipped" "Release binary or validator build failed."))
        $stages.Add((New-Result "windows-office-edit-smoke" "skipped" "Release binary or validator build failed."))
        $stages.Add((New-Result "canonical-recipes" "skipped" "Release binary or validator build failed."))
    }
}
else {
    $stages.Add((New-Result "release-build" "skipped" "A required prerequisite failed."))
    $stages.Add((New-Result "openxml-validator-build" "skipped" "A required prerequisite failed."))
    $stages.Add((New-Result "mutation-contract-evidence" "skipped" "A required prerequisite failed."))
    $stages.Add((New-Result "windows-office-edit-smoke" "skipped" "A required prerequisite failed."))
    $stages.Add((New-Result "canonical-recipes" "skipped" "A required prerequisite failed."))
}

$scenarioFailures = @($scenarios | Where-Object { (Get-LegionScenarioStatus $_ ([bool]$SkipOffice)) -ne "passed" })
$recipeFailures = @($recipes | Where-Object {
    $_.build.status -ne "passed" -or $_.strictValidation.status -ne "passed" -or $_.openXmlSdk.status -ne "passed" -or ((-not $SkipOffice) -and $_.office.status -ne "passed")
})
$failedRequiredStages = @($stages | Where-Object { $_.status -eq "failed" }).Count
$status = if ($requiredPrerequisiteFailures.Count -eq 0 -and $failedRequiredStages -eq 0 -and $scenarioFailures.Count -eq 0 -and $recipeFailures.Count -eq 0 -and $recipes.Count -eq 5) { "passed" } else { "failed" }
$platform = "non-windows"
if ($script:IsWindowsPlatform) { $platform = "windows" }
$proofBoundary = "Strict, conformance, and Open XML SDK proof only. Microsoft Office COM was skipped."
if (-not $SkipOffice) {
    $proofBoundary = "Includes bounded desktop Microsoft Office COM open/save proof for the five canonical recipes and COM-open proof for contract scenarios."
}
$scenarioTotal = $scenarios.Count
$recipeTotal = $recipes.Count
$scenarioPassed = $scenarioTotal - $scenarioFailures.Count
$recipePassed = $recipeTotal - $recipeFailures.Count
$prerequisiteFailureCount = $requiredPrerequisiteFailures.Count
$summaryCounts = [pscustomobject]@{
    scenariosTotal = $scenarioTotal
    scenariosPassed = $scenarioPassed
    recipesTotal = $recipeTotal
    recipesPassed = $recipePassed
    failedRequiredStages = $failedRequiredStages
    prerequisiteFailures = $prerequisiteFailureCount
}
$summaryPrerequisites = $prerequisites.ToArray()
$summaryStages = $stages.ToArray()
$summaryScenarios = @($scenarios)
$summaryRecipes = $recipes.ToArray()
$summary = [pscustomobject]@{
    schemaVersion  = "ooxml-cli.legion-proof.v1"
    generatedAtUtc = [DateTime]::UtcNow.ToString("o")
    status         = $status
    host           = [Environment]::MachineName
    platform       = $platform
    skipOffice     = [bool]$SkipOffice
    outputDir      = $output
    binary         = $BinaryPath
    validator      = $validatorDll
    prerequisites  = $summaryPrerequisites
    stages         = $summaryStages
    scenarios      = $summaryScenarios
    recipes        = $summaryRecipes
    counts         = $summaryCounts
    proofBoundary  = $proofBoundary
}
Write-Utf8NoBom -Path $summaryPath -Content (($summary | ConvertTo-Json -Depth 12) + [Environment]::NewLine)
Write-LegionMarkdownReport -Summary $summary -Path $ReportPath
Write-Host ("Legion proof status: {0}" -f $status)
Write-Host ("Summary JSON: {0}" -f $summaryPath)
Write-Host ("Markdown report: {0}" -f $ReportPath)
if ($status -ne "passed") { exit 1 }
