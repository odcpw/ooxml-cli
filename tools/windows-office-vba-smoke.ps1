[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path,

    [string]$OutputDir = (Join-Path $env:TEMP ("ooxml-office-vba-smoke-" + [guid]::NewGuid().ToString("N"))),

    [string]$BinaryPath = "",

    [string]$GoExe = "",

    [string]$DotNetExe = "",

    [string]$OpenXmlValidatorProject = "",

    [int]$OfficeOracleTimeoutSeconds = 120,

    [switch]$SkipBuild,

    [switch]$SkipOpenXmlSdk,

    [switch]$RequireOpenXmlSdk,

    [switch]$RunConformance,

    [switch]$SkipOffice,

    [switch]$EnableVbaObjectModelAccess,

    [switch]$Visible,

    [switch]$WriteArtifactProofMatrix,

    [switch]$FailOnArtifactProofGap,

    [string]$ArtifactProofMatrixJson = "",

    [string]$ArtifactProofMatrixMarkdown = "",

    [string]$OfficeEditSmokeSummaryPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-GoExe {
    param([string]$Requested)
    if ($Requested -ne "") { return $Requested }
    $fromPath = Get-Command go -ErrorAction SilentlyContinue
    if ($null -ne $fromPath) { return $fromPath.Source }
    $default = "C:\Program Files\Go\bin\go.exe"
    if (Test-Path -LiteralPath $default -PathType Leaf) { return $default }
    throw "Go executable not found. Pass -GoExe or install Go."
}

function Resolve-DotNetExe {
    param([string]$Requested)
    if ($Requested -ne "") { return $Requested }
    $fromPath = Get-Command dotnet -ErrorAction SilentlyContinue
    if ($null -ne $fromPath) { return $fromPath.Source }
    $default = "C:\Program Files\dotnet\dotnet.exe"
    if (Test-Path -LiteralPath $default -PathType Leaf) { return $default }
    return ""
}

function Quote-Argument {
    param([string]$Value)
    if ($Value -eq "") { return '""' }
    if ($Value -match '[\s"]') { return '"' + ($Value -replace '"', '\"') + '"' }
    return $Value
}

function Format-CommandLine {
    param([string]$FilePath, [string[]]$Arguments)
    return ((@($FilePath) + $Arguments) | ForEach-Object { Quote-Argument -Value $_ }) -join " "
}

function Invoke-Process {
    param([string]$FilePath, [string[]]$Arguments)
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $oldErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = @(& $FilePath @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $oldErrorActionPreference
    }
    $timer.Stop()
    [pscustomobject]@{
        exitCode  = $exitCode
        output    = (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine)
        elapsedMs = $timer.ElapsedMilliseconds
        command   = (Format-CommandLine -FilePath $FilePath -Arguments $Arguments)
    }
}

function Get-OfficeProcessNamesForFamily {
    param([string]$Family)

    switch ($Family) {
        "xlsx" { return @("EXCEL") }
        "pptx" { return @("POWERPNT") }
        default { return @("EXCEL", "POWERPNT", "WINWORD") }
    }
}

function Get-ProcessIdSet {
    param([string[]]$Names)

    $ids = @{}
    foreach ($process in @(Get-Process -Name $Names -ErrorAction SilentlyContinue)) {
        $ids[[int]$process.Id] = $true
    }
    return $ids
}

function Stop-NewOfficeProcesses {
    param(
        [string[]]$Names,
        [hashtable]$ExistingIds,
        [datetime]$StartedAt
    )

    foreach ($process in @(Get-Process -Name $Names -ErrorAction SilentlyContinue)) {
        if ($ExistingIds.ContainsKey([int]$process.Id)) {
            continue
        }
        try {
            if ($null -ne $process.StartTime -and $process.StartTime -lt $StartedAt.AddSeconds(-2)) {
                continue
            }
        }
        catch {
            continue
        }
        try {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
        catch {}
    }
}

function Stop-ProcessTree {
    param([int]$ProcessId)

    foreach ($child in @(Get-CimInstance Win32_Process -Filter ("ParentProcessId = {0}" -f $ProcessId) -ErrorAction SilentlyContinue)) {
        Stop-ProcessTree -ProcessId ([int]$child.ProcessId)
    }
    try {
        Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
    }
    catch {}
}

function Read-TextFileBestEffort {
    param([string]$Path)

    try {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            return (Get-Content -LiteralPath $Path -Raw)
        }
    }
    catch {}
    return ""
}

function Invoke-OfficeBackedProcess {
    param(
        [string]$FilePath,
        [string[]]$Arguments,
        [string]$Family,
        [int]$TimeoutSeconds
    )

    if ($TimeoutSeconds -le 0) {
        return (Invoke-Process -FilePath $FilePath -Arguments $Arguments)
    }

    if ($script:ProcessCaptureDir -eq "") {
        $script:ProcessCaptureDir = [System.IO.Path]::GetTempPath()
    }
    $script:ProcessCaptureIndex++
    $capturePrefix = Join-Path $script:ProcessCaptureDir ("office-backed-{0}" -f $script:ProcessCaptureIndex)
    $stdoutPath = "$capturePrefix.stdout.txt"
    $stderrPath = "$capturePrefix.stderr.txt"
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue

    $officeProcessNames = @(Get-OfficeProcessNamesForFamily -Family $Family)
    $officeProcessIdsBefore = Get-ProcessIdSet -Names $officeProcessNames
    $startedAt = Get-Date
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $argumentLine = ($Arguments | ForEach-Object { Quote-Argument -Value $_ }) -join " "
    $process = Start-Process -FilePath $FilePath -ArgumentList $argumentLine -WorkingDirectory $root -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -WindowStyle Hidden -PassThru
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    $timer.Stop()

    if (-not $finished) {
        Stop-ProcessTree -ProcessId $process.Id
        Stop-NewOfficeProcesses -Names $officeProcessNames -ExistingIds $officeProcessIdsBefore -StartedAt $startedAt
        $stdout = Read-TextFileBestEffort -Path $stdoutPath
        $stderr = Read-TextFileBestEffort -Path $stderrPath
        $output = (($stdout, $stderr | Where-Object { $null -ne $_ -and [string]$_ -ne "" }) -join [Environment]::NewLine).Trim()
        return [pscustomobject]@{
            exitCode  = 124
            output    = ("Office-backed process exceeded {0} second(s). {1}" -f $TimeoutSeconds, $output).Trim()
            elapsedMs = $timer.ElapsedMilliseconds
            command   = (Format-CommandLine -FilePath $FilePath -Arguments $Arguments)
        }
    }

    try {
        $process.WaitForExit()
        $process.Refresh()
    }
    catch {}
    $exitCode = $process.ExitCode
    if ($null -eq $exitCode) {
        $exitCode = 0
    }
    if ($exitCode -ne 0) {
        Stop-NewOfficeProcesses -Names $officeProcessNames -ExistingIds $officeProcessIdsBefore -StartedAt $startedAt
    }
    $stdout = Read-TextFileBestEffort -Path $stdoutPath
    $stderr = Read-TextFileBestEffort -Path $stderrPath
    $output = (($stdout, $stderr | Where-Object { $null -ne $_ -and [string]$_ -ne "" }) -join [Environment]::NewLine).Trim()

    [pscustomobject]@{
        exitCode  = $exitCode
        output    = $output
        elapsedMs = $timer.ElapsedMilliseconds
        command   = (Format-CommandLine -FilePath $FilePath -Arguments $Arguments)
    }
}

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments, [string]$Label)
    Write-Host ("[{0}] {1}" -f $Label, (Format-CommandLine -FilePath $FilePath -Arguments $Arguments))
    $result = Invoke-Process -FilePath $FilePath -Arguments $Arguments
    if ($result.exitCode -ne 0) {
        throw ("{0} failed with exit code {1}. {2}" -f $Label, $result.exitCode, $result.output)
    }
    return $result
}

function Invoke-OfficeBackedChecked {
    param([string]$FilePath, [string[]]$Arguments, [string]$Label, [string]$Family)
    Write-Host ("[{0}] {1}" -f $Label, (Format-CommandLine -FilePath $FilePath -Arguments $Arguments))
    $result = Invoke-OfficeBackedProcess -FilePath $FilePath -Arguments $Arguments -Family $Family -TimeoutSeconds $OfficeOracleTimeoutSeconds
    if ($result.exitCode -ne 0) {
        throw ("{0} failed with exit code {1}. {2}" -f $Label, $result.exitCode, $result.output)
    }
    return $result
}

function New-Stage {
    param([string]$Status, [string]$Detail = "", [string]$Command = "", [string]$Artifact = "", [object]$ElapsedMs = 0)
    [pscustomobject]@{
        status    = $Status
        detail    = $Detail
        command   = $Command
        artifact  = $Artifact
        elapsedMs = [int64]$ElapsedMs
    }
}

function Write-AsciiFile {
    param([string]$Path, [string[]]$Lines)
    ($Lines -join "`r`n") + "`r`n" | Set-Content -LiteralPath $Path -Encoding ASCII -NoNewline
}

function Invoke-StrictValidation {
    param([string]$Path)
    $result = Invoke-Process -FilePath $BinaryPath -Arguments @("--json", "validate", $Path, "--strict")
    if ($result.exitCode -eq 0) {
        return New-Stage -Status "passed" -Detail "ooxml validate --strict accepted the file." -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
    }
    return New-Stage -Status "failed" -Detail $result.output -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
}

function Invoke-OpenXmlValidation {
    param([string]$Path)
    if (-not $script:RunOpenXmlSdk) {
        $status = if ($RequireOpenXmlSdk) { "failed" } else { "not-run" }
        return New-Stage -Status $status -Detail "Open XML SDK validation was not available." -Artifact $Path
    }
    $result = Invoke-Process -FilePath $script:DotNet -Arguments @($script:OpenXmlValidatorDll, "--json", $Path)
    if ($result.exitCode -eq 0) {
        return New-Stage -Status "passed" -Detail "Microsoft Open XML SDK validator reported 0 schema errors." -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
    }
    return New-Stage -Status "failed" -Detail $result.output -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
}

function Invoke-PackageReadback {
    param([string]$Path, [string]$Family)
    $result = Invoke-Process -FilePath $BinaryPath -Arguments @("--json", "inspect", $Path)
    if ($result.exitCode -ne 0) {
        return New-Stage -Status "failed" -Detail $result.output -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
    }
    try {
        $doc = $result.output | ConvertFrom-Json
        $actualFamily = [string]$doc.type
        if ($actualFamily -ne $Family) {
            return New-Stage -Status "failed" -Detail ("ooxml inspect reported family {0}; expected {1}." -f $actualFamily, $Family) -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
        }
    }
    catch {
        return New-Stage -Status "failed" -Detail "ooxml inspect returned invalid JSON." -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
    }
    return New-Stage -Status "passed" -Detail ("ooxml inspect read the saved {0} package." -f $Family) -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
}

function Invoke-ConformanceCheck {
    param([string]$Path)
    if (-not $RunConformance) {
        return New-Stage -Status "not-run" -Detail "Run with -RunConformance to include repair invariant checks." -Artifact $Path
    }
    $result = Invoke-Process -FilePath $BinaryPath -Arguments @("--json", "conformance", "check", $Path)
    if ($result.exitCode -eq 0) {
        return New-Stage -Status "passed" -Detail "ooxml conformance check accepted the file." -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
    }
    return New-Stage -Status "failed" -Detail $result.output -Command $result.command -Artifact $Path -ElapsedMs $result.elapsedMs
}

function Add-FileScenario {
    param(
        [string]$Name,
        [string]$Family,
        [string]$Path,
        [string]$CommandPath = "",
        [string]$ExactCommand = "",
        [string]$InputFixtureType = "office-created macro fixture"
    )
    $readback = Invoke-PackageReadback -Path $Path -Family $Family
    $strict = Invoke-StrictValidation -Path $Path
    $conformance = if ($readback.status -eq "passed" -and $strict.status -eq "passed") {
        Invoke-ConformanceCheck -Path $Path
    }
    else {
        New-Stage -Status "skipped" -Detail "Readback or strict validation failed." -Artifact $Path
    }
    $openXml = if ($conformance.status -ne "failed") {
        Invoke-OpenXmlValidation -Path $Path
    }
    else {
        New-Stage -Status "skipped" -Detail "Conformance check failed." -Artifact $Path
    }
    $script:Scenarios.Add([pscustomobject]@{
        name            = $Name
        family          = $Family
        file            = $Path
        commandPath     = $CommandPath
        exactCommand    = $ExactCommand
        inputFixtureType = $InputFixtureType
        proofLevel      = "pending-office"
        readback        = $readback
        strict          = $strict
        conformance     = $conformance
        openXmlSdk      = $openXml
        microsoftOffice = (New-Stage -Status "pending" -Detail "Waiting for desktop Office COM oracle.")
    })
    if ($readback.status -eq "passed" -and $strict.status -eq "passed" -and $conformance.status -ne "failed" -and $openXml.status -ne "failed") {
        $script:OfficeInputs.Add($Path)
    }
}

function Add-GuardScenario {
    param(
        [string]$Name,
        [string]$Family,
        [string[]]$Arguments,
        [string]$OutputPath,
        [string]$CommandPath = ""
    )
    $result = Invoke-Process -FilePath $BinaryPath -Arguments $Arguments
    $passed = $result.exitCode -ne 0 -and
        $result.output -match "version-dependent _VBA_PROJECT metadata" -and
        (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf))
    $script:Scenarios.Add([pscustomobject]@{
        name            = $Name
        family          = $Family
        file            = $OutputPath
        commandPath     = $CommandPath
        exactCommand    = $result.command
        inputFixtureType = "guarded macro fixture"
        proofLevel      = if ($passed) { "guarded-refusal" } else { "failed" }
        readback        = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        strict          = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        conformance     = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        openXmlSdk      = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        microsoftOffice = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        guard           = if ($passed) {
            New-Stage -Status "passed" -Detail "Command refused an Office-shaped module-set change before writing output." -Command $result.command -ElapsedMs $result.elapsedMs
        } else {
            New-Stage -Status "failed" -Detail ("Expected guarded refusal, got exit {0}: {1}" -f $result.exitCode, $result.output) -Command $result.command -ElapsedMs $result.elapsedMs
        }
    })
}

function New-ProofTierFromStage {
    param(
        [object]$Stage,
        [string]$PassedDetail,
        [string]$FallbackDetail,
        [string[]]$ExtraEvidence = @()
    )

    if ($null -eq $Stage) { return $null }
    $status = [string]$Stage.status
    if ($status -eq "" -or $status -eq "not-run" -or $status -eq "skipped") {
        return $null
    }

    $detail = [string]$Stage.detail
    if ($status -eq "passed" -and $PassedDetail -ne "") {
        $detail = $PassedDetail
    }
    elseif ($detail -eq "") {
        $detail = $FallbackDetail
    }

    $evidence = New-Object System.Collections.Generic.List[string]
    foreach ($value in @($ExtraEvidence)) {
        if ($null -ne $value -and [string]$value -ne "") {
            [void]$evidence.Add([string]$value)
        }
    }
    foreach ($name in @("command", "artifact")) {
        if ($Stage.PSObject.Properties.Name -contains $name) {
            $value = $Stage.$name
            if ($null -ne $value -and [string]$value -ne "") {
                [void]$evidence.Add([string]$value)
            }
        }
    }

    [pscustomobject][ordered]@{
        status = $status
        detail = $detail
        evidence = @($evidence.ToArray())
    }
}

function New-WaivedProofTier {
    param(
        [string]$Detail,
        [string[]]$Evidence = @()
    )

    [pscustomobject][ordered]@{
        status = "waived"
        detail = $Detail
        evidence = @($Evidence | Where-Object { $null -ne $_ -and [string]$_ -ne "" })
    }
}

function New-VbaProofEvidence {
    param([string]$SummaryPath)

    $proofs = New-Object System.Collections.Generic.List[object]
    $scenarios = if ($script:Scenarios -is [System.Array]) {
        $script:Scenarios
    }
    elseif ($null -ne $script:Scenarios -and ($script:Scenarios.PSObject.Methods.Name -contains "ToArray")) {
        $script:Scenarios.ToArray()
    }
    else {
        @($script:Scenarios)
    }
    foreach ($scenario in $scenarios) {
        if (-not ($scenario.PSObject.Properties.Name -contains "commandPath")) { continue }
        $commandPath = [string]$scenario.commandPath
        $file = [string]$scenario.file
        if ($commandPath -eq "") {
            continue
        }

        $guard = if ($scenario.PSObject.Properties.Name -contains "guard") { $scenario.guard } else { $null }
        $guardPassed = $null -ne $guard -and [string]$guard.status -eq "passed"
        $fileExists = $file -ne "" -and (Test-Path -LiteralPath $file -PathType Leaf)
        if (-not $fileExists -and -not $guardPassed) {
            continue
        }

        $baseEvidence = @("windows-office-vba-smoke summary: $SummaryPath", "scenario: $($scenario.name)")
        if ($file -ne "") {
            $baseEvidence += "blocked output: $file"
        }
        $tiers = [ordered]@{}
        if ($guardPassed) {
            $guardEvidence = @($baseEvidence + @($guard.command, $guard.detail))
            $waivedDetail = "Command refused to write an Office-shaped macro source rewrite before producing an artifact."
            $tiers.structural = New-WaivedProofTier -Detail $waivedDetail -Evidence $guardEvidence
            $tiers.readback = New-WaivedProofTier -Detail $waivedDetail -Evidence $guardEvidence
            $tiers.validate = New-WaivedProofTier -Detail $waivedDetail -Evidence $guardEvidence
            $tiers.conformance = New-WaivedProofTier -Detail $waivedDetail -Evidence $guardEvidence
            $tiers.office = New-WaivedProofTier -Detail $waivedDetail -Evidence $guardEvidence
        }
        else {
            $baseEvidence += "output: $file"
            $tiers.structural = New-ProofTierFromStage `
                -Stage $scenario.openXmlSdk `
                -PassedDetail "Microsoft Open XML SDK schema validation passed for the macro smoke output." `
                -FallbackDetail "Microsoft Open XML SDK schema validation did not pass for the macro smoke output." `
                -ExtraEvidence $baseEvidence
            $tiers.readback = New-ProofTierFromStage `
                -Stage $scenario.readback `
                -PassedDetail "ooxml inspect read the saved macro smoke output." `
                -FallbackDetail "ooxml inspect did not read the saved macro smoke output." `
                -ExtraEvidence $baseEvidence
            $tiers.validate = New-ProofTierFromStage `
                -Stage $scenario.strict `
                -PassedDetail "ooxml validate --strict accepted the macro smoke output." `
                -FallbackDetail "ooxml validate --strict did not accept the macro smoke output." `
                -ExtraEvidence $baseEvidence
            $tiers.conformance = New-ProofTierFromStage `
                -Stage $scenario.conformance `
                -PassedDetail "ooxml conformance check accepted the macro smoke output." `
                -FallbackDetail "ooxml conformance check did not accept the macro smoke output." `
                -ExtraEvidence $baseEvidence
            $tiers.office = New-ProofTierFromStage `
                -Stage $scenario.microsoftOffice `
                -PassedDetail "Desktop Microsoft Office opened the macro smoke output without repair/failure." `
                -FallbackDetail "Desktop Microsoft Office did not open the macro smoke output cleanly." `
                -ExtraEvidence $baseEvidence
        }

        foreach ($tierName in @("structural", "readback", "validate", "conformance", "office")) {
            if ($null -eq $tiers[$tierName]) {
                $tiers.Remove($tierName)
            }
        }

        [void]$proofs.Add([pscustomobject][ordered]@{
            commandPath = $commandPath
            inputFixtureType = [string]$scenario.inputFixtureType
            generatedOutputPath = if ($fileExists) { $file } else { "" }
            exactCommand = [string]$scenario.exactCommand
            sourceSummary = $SummaryPath
            scenarioName = [string]$scenario.name
            tiers = [pscustomobject]$tiers
        })
    }

    [pscustomobject][ordered]@{
        schemaVersion = "ooxml-cli.vba-smoke-evidence.v1"
        proofs = @($proofs.ToArray())
    }
}

function Add-OfficeResultToMap {
    param([hashtable]$Map, [object]$OfficeResult)
    if ($null -eq $OfficeResult) { return }
    if ($OfficeResult -is [System.Array]) {
        foreach ($inner in $OfficeResult) {
            Add-OfficeResultToMap -Map $Map -OfficeResult $inner
        }
        return
    }
    if ($null -eq $OfficeResult.file -or [string]$OfficeResult.file -eq "") { return }
    $key = ([string]$OfficeResult.file).Trim().ToLowerInvariant()
    $Map[$key] = $OfficeResult
}

$root = (Resolve-Path -LiteralPath $RepoRoot).Path
$outRoot = [System.IO.Path]::GetFullPath($OutputDir)
$binDir = Join-Path $outRoot "bin"
$seedDir = Join-Path $outRoot "seeds"
$sourceDir = Join-Path $outRoot "sources"
$caseDir = Join-Path $outRoot "outputs"
$oracleDir = Join-Path $outRoot "office-oracle"
$processCaptureDir = Join-Path $outRoot "process-captures"
New-Item -ItemType Directory -Force -Path $outRoot, $binDir, $seedDir, $sourceDir, $caseDir, $oracleDir, $processCaptureDir | Out-Null
$script:ProcessCaptureDir = $processCaptureDir
$script:ProcessCaptureIndex = 0

$script:DotNet = Resolve-DotNetExe -Requested $DotNetExe
$explicitBinaryPath = $BinaryPath -ne ""
if ($BinaryPath -eq "") {
    $BinaryPath = Join-Path $binDir "ooxml.exe"
}
if (-not $SkipBuild) {
    if ($explicitBinaryPath) {
        throw "Refusing to build Go into explicit -BinaryPath. Omit -BinaryPath to build the Go CLI, or pass -SkipBuild to test the existing binary at: $BinaryPath"
    }
    $go = Resolve-GoExe -Requested $GoExe
    Invoke-Checked -FilePath $go -Arguments @("build", "-buildvcs=false", "-o", $BinaryPath, (Join-Path $root "cmd\ooxml")) -Label "build" | Out-Null
}
elseif (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "BinaryPath does not exist and -SkipBuild was set: $BinaryPath"
}

if ($OpenXmlValidatorProject -eq "") {
    $OpenXmlValidatorProject = Join-Path $root "tools\openxml-validator\openxml-validator.csproj"
}
$script:RunOpenXmlSdk = $false
$script:OpenXmlValidatorDll = ""
$openXmlSetup = New-Stage -Status "not-run" -Detail "Open XML SDK validation was skipped."
if (-not $SkipOpenXmlSdk) {
    if ($script:DotNet -eq "") {
        $openXmlSetup = New-Stage -Status "not-run" -Detail "dotnet was not found on PATH."
    }
    elseif (-not (Test-Path -LiteralPath $OpenXmlValidatorProject -PathType Leaf)) {
        $openXmlSetup = New-Stage -Status "not-run" -Detail "Open XML validator project was not found: $OpenXmlValidatorProject"
    }
    else {
        $validatorRoot = Split-Path -Parent $OpenXmlValidatorProject
        $script:OpenXmlValidatorDll = Join-Path $validatorRoot "bin\Release\net8.0\openxml-validator.dll"
        $nugetConfig = Join-Path $outRoot "NuGet.Config"
        Set-Content -LiteralPath $nugetConfig -Encoding UTF8 -Value @(
            '<?xml version="1.0" encoding="utf-8"?>',
            '<configuration>',
            '  <packageSources>',
            '    <clear />',
            '    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />',
            '  </packageSources>',
            '</configuration>'
        )
        $oldAppData = $env:APPDATA
        $oldLocalAppData = $env:LOCALAPPDATA
        $env:APPDATA = Join-Path $outRoot "dotnet-appdata"
        $env:LOCALAPPDATA = Join-Path $outRoot "dotnet-localappdata"
        New-Item -ItemType Directory -Force -Path $env:APPDATA, $env:LOCALAPPDATA | Out-Null
        try {
            Invoke-Checked -FilePath $script:DotNet -Arguments @("restore", $OpenXmlValidatorProject, "--configfile", $nugetConfig, "--nologo") -Label "openxml-sdk:restore" | Out-Null
            Invoke-Checked -FilePath $script:DotNet -Arguments @("build", $OpenXmlValidatorProject, "-c", "Release", "--no-restore", "--nologo") -Label "openxml-sdk:build" | Out-Null
            if (Test-Path -LiteralPath $script:OpenXmlValidatorDll -PathType Leaf) {
                $script:RunOpenXmlSdk = $true
                $openXmlSetup = New-Stage -Status "passed" -Detail "Open XML SDK validator built successfully." -Artifact $script:OpenXmlValidatorDll
            }
        }
        catch {
            if ($RequireOpenXmlSdk) { throw }
            $openXmlSetup = New-Stage -Status "not-run" -Detail ("Open XML SDK validator build failed: {0}" -f $_.Exception.Message)
            Write-Warning $openXmlSetup.detail
        }
        finally {
            $env:APPDATA = $oldAppData
            $env:LOCALAPPDATA = $oldLocalAppData
        }
    }
}
if ($RequireOpenXmlSdk -and -not $script:RunOpenXmlSdk) {
    throw "Open XML SDK validation was required but is not available."
}

$standardSource = Join-Path $sourceDir "SeedModule.bas"
$classSource = Join-Path $sourceDir "SeedClass.cls"
$replacementSource = Join-Path $sourceDir "SeedModuleReplacement.bas"
$agentSource = Join-Path $sourceDir "AgentSmoke.bas"
Write-AsciiFile -Path $standardSource -Lines @(
    'Attribute VB_Name = "SeedModule"',
    'Public Sub SeedMacro()',
    '    Debug.Print "seed"',
    'End Sub'
)
Write-AsciiFile -Path $classSource -Lines @(
    'VERSION 1.0 CLASS',
    'BEGIN',
    '  MultiUse = -1',
    'END',
    'Attribute VB_Name = "SeedClass"',
    'Attribute VB_GlobalNameSpace = False',
    'Attribute VB_Creatable = False',
    'Attribute VB_PredeclaredId = False',
    'Attribute VB_Exposed = False',
    'Public Function Ping() As String',
    '    Ping = "ok"',
    'End Function'
)
Write-AsciiFile -Path $replacementSource -Lines @(
    'Attribute VB_Name = "SeedModule"',
    'Public Sub SeedMacro()',
    '    Debug.Print "seed replaced by ooxml-cli"',
    'End Sub'
)
Write-AsciiFile -Path $agentSource -Lines @(
    'Attribute VB_Name = "AgentSmoke"',
    'Public Sub AgentSmokeRun()',
    '    Debug.Print "guarded add"',
    'End Sub'
)

$xlsmSeed = Join-Path $seedDir "seed.xlsm"
$pptmSeed = Join-Path $seedDir "seed.pptm"
$createScript = Join-Path $root "tools\windows-office-vba-create.ps1"
Write-Host "[seed] creating Excel and PowerPoint macro-enabled seeds through ooxml vba create"
$seedPrefixArgs = @("--format", "json", "vba", "create")
$seedTailArgs = @(
    "--source", $standardSource,
    "--source", $classSource,
    "--office-create-script", $createScript,
    "--force"
)
if ($EnableVbaObjectModelAccess) {
    $seedTailArgs += "--enable-vba-object-model-access"
}
if ($Visible) {
    $seedTailArgs += "--visible"
}
$xlsmCreateResult = Invoke-OfficeBackedChecked -FilePath $BinaryPath -Arguments ($seedPrefixArgs + @($xlsmSeed, "--family", "xlsx") + $seedTailArgs) -Label "seed:xlsm:create" -Family "xlsx"
$pptmCreateResult = Invoke-OfficeBackedChecked -FilePath $BinaryPath -Arguments ($seedPrefixArgs + @($pptmSeed, "--family", "pptx") + $seedTailArgs) -Label "seed:pptm:create" -Family "pptx"

$script:Scenarios = New-Object System.Collections.Generic.List[object]
$script:OfficeInputs = New-Object System.Collections.Generic.List[string]

$pureXlsxBase = Join-Path $caseDir "pure-base.xlsx"
$purePptxBase = Join-Path $caseDir "pure-base.pptx"
$pureDocxBase = Join-Path $caseDir "pure-base.docx"

Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "xlsx", "scaffold", $pureXlsxBase, "--force") -Label "pure:xlsx:scaffold" | Out-Null
Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "pptx", "scaffold", $purePptxBase, "--title", "Pure VBA Deck", "--subtitle", "Office proof", "--force") -Label "pure:pptx:scaffold" | Out-Null
Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "docx", "scaffold", $pureDocxBase, "--text", "Pure VBA document", "--force") -Label "pure:docx:scaffold" | Out-Null

$pureScenarios = @(
    [pscustomobject]@{
        name = "vba-xlsm-pure-standard-from-scaffold"; family = "xlsx"; input = $pureXlsxBase; output = (Join-Path $caseDir "pure-standard.xlsm"); sources = @($standardSource)
    },
    [pscustomobject]@{
        name = "vba-xlsm-pure-class-from-scaffold"; family = "xlsx"; input = $pureXlsxBase; output = (Join-Path $caseDir "pure-class.xlsm"); sources = @($standardSource, $classSource)
    },
    [pscustomobject]@{
        name = "vba-pptm-pure-standard-from-scaffold"; family = "pptx"; input = $purePptxBase; output = (Join-Path $caseDir "pure-standard.pptm"); sources = @($standardSource)
    },
    [pscustomobject]@{
        name = "vba-pptm-pure-class-from-scaffold"; family = "pptx"; input = $purePptxBase; output = (Join-Path $caseDir "pure-class.pptm"); sources = @($standardSource, $classSource)
    },
    [pscustomobject]@{
        name = "vba-docm-pure-standard-from-scaffold"; family = "docx"; input = $pureDocxBase; output = (Join-Path $caseDir "pure-standard.docm"); sources = @($standardSource)
    }
)

foreach ($pure in $pureScenarios) {
    $args = @("--format", "json", "vba", "create", $pure.input, "--pure", "--family", $pure.family)
    foreach ($source in @($pure.sources)) {
        $args += @("--source", $source)
    }
    $args += @("--out", $pure.output)
    $pureResult = Invoke-Checked -FilePath $BinaryPath -Arguments $args -Label ("pure:{0}:create" -f $pure.name)
    Add-FileScenario -Name $pure.name -Family $pure.family -Path $pure.output -CommandPath "ooxml vba create" -ExactCommand $pureResult.command -InputFixtureType "pure Rust authored macro package"
}

$families = @(
    [pscustomobject]@{
        family = "xlsx"; macroFamily = "xlsm"; base = (Join-Path $root "testdata\xlsx\minimal-workbook\workbook.xlsx"); seed = $xlsmSeed; createCommand = $xlsmCreateResult.command; attached = (Join-Path $caseDir "attached.xlsm"); removed = (Join-Path $caseDir "removed.xlsx"); converted = (Join-Path $caseDir "converted-alias.xlsx"); replaced = (Join-Path $caseDir "replaced.xlsm"); addBlocked = (Join-Path $caseDir "add-blocked.xlsm"); removeBlocked = (Join-Path $caseDir "remove-blocked.xlsm")
    },
    [pscustomobject]@{
        family = "pptx"; macroFamily = "pptm"; base = (Join-Path $root "testdata\pptx\minimal-title\presentation.pptx"); seed = $pptmSeed; createCommand = $pptmCreateResult.command; attached = (Join-Path $caseDir "attached.pptm"); removed = (Join-Path $caseDir "removed.pptx"); converted = ""; replaced = (Join-Path $caseDir "replaced.pptm"); addBlocked = (Join-Path $caseDir "add-blocked.pptm"); removeBlocked = (Join-Path $caseDir "remove-blocked.pptm")
    }
)

foreach ($item in $families) {
    Add-FileScenario -Name ("vba-{0}-office-seed" -f $item.macroFamily) -Family $item.family -Path $item.seed -CommandPath "ooxml vba create" -ExactCommand $item.createCommand -InputFixtureType "office-created macro seed"

    $binPath = Join-Path $caseDir ("{0}-vbaProject.bin" -f $item.macroFamily)
    Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "extract-bin", $item.seed, "--out", $binPath) -Label ("vba:{0}:extract-bin" -f $item.macroFamily) | Out-Null
    $attachResult = Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "attach", $item.base, "--bin", $binPath, "--out", $item.attached) -Label ("vba:{0}:attach" -f $item.macroFamily)
    Add-FileScenario -Name ("vba-{0}-attach-office-bin" -f $item.macroFamily) -Family $item.family -Path $item.attached -CommandPath "ooxml vba attach" -ExactCommand $attachResult.command -InputFixtureType "office-authored macro bin"

    $removeResult = Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "remove", $item.attached, "--out", $item.removed) -Label ("vba:{0}:remove-project" -f $item.macroFamily)
    Add-FileScenario -Name ("vba-{0}-remove-project" -f $item.macroFamily) -Family $item.family -Path $item.removed -CommandPath "ooxml vba remove" -ExactCommand $removeResult.command -InputFixtureType "office-authored macro package"

    if ($item.macroFamily -eq "xlsm") {
        $convertResult = Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "convert", "xlsm-to-xlsx", $item.attached, "--out", $item.converted) -Label "vba:xlsm:convert-xlsm-to-xlsx"
        Add-FileScenario -Name "vba-xlsm-convert-xlsm-to-xlsx" -Family $item.family -Path $item.converted -CommandPath "ooxml convert xlsm-to-xlsx" -ExactCommand $convertResult.command -InputFixtureType "office-authored macro package"
    }

    $list = (& $BinaryPath --format json vba list $item.seed | ConvertFrom-Json)
    $standard = $list.project.modules | Where-Object { $_.name -eq "SeedModule" } | Select-Object -First 1
    $classModule = $list.project.modules | Where-Object { $_.name -eq "SeedClass" } | Select-Object -First 1
    if ($null -eq $standard -or $null -eq $classModule) {
        throw ("{0} seed did not expose SeedModule and SeedClass through vba list." -f $item.macroFamily)
    }
    Add-GuardScenario -Name ("vba-{0}-replace-module-guard" -f $item.macroFamily) -Family $item.family -Arguments @("--format", "json", "vba", "replace-module", $item.seed, "--module", $standard.primarySelector, "--source", $replacementSource, "--expect-sha256", $standard.sha256, "--allow-experimental-vba-source-rewrite", "--out", $item.replaced) -OutputPath $item.replaced -CommandPath "ooxml vba replace-module"

    Add-GuardScenario -Name ("vba-{0}-add-module-guard" -f $item.macroFamily) -Family $item.family -Arguments @("--format", "json", "vba", "add-module", $item.seed, "--source", $agentSource, "--allow-experimental-vba-source-rewrite", "--out", $item.addBlocked) -OutputPath $item.addBlocked -CommandPath "ooxml vba add-module"
    Add-GuardScenario -Name ("vba-{0}-remove-module-guard" -f $item.macroFamily) -Family $item.family -Arguments @("--format", "json", "vba", "remove-module", $item.seed, "--module", $classModule.primarySelector, "--expect-sha256", $classModule.sha256, "--allow-experimental-vba-source-rewrite", "--out", $item.removeBlocked) -OutputPath $item.removeBlocked -CommandPath "ooxml vba remove-module"
}

$oracleSummaryPath = Join-Path $oracleDir "summary.json"
if ($SkipOffice) {
    foreach ($scenario in $script:Scenarios) {
        if ($scenario.microsoftOffice.status -eq "pending") {
            $scenario.microsoftOffice = New-Stage -Status "skipped" -Detail "Skipped by -SkipOffice."
            if ($scenario.openXmlSdk.status -eq "passed") { $scenario.proofLevel = "openxml-sdk-schema" }
            elseif ($scenario.conformance.status -eq "passed") { $scenario.proofLevel = "repair-conformance" }
            elseif ($scenario.strict.status -eq "passed") { $scenario.proofLevel = "strict-validation" }
        }
    }
}
elseif ($script:OfficeInputs.Count -gt 0) {
    $oracle = Join-Path $root "tools\windows-office-oracle.ps1"
    Write-Host ("[office-oracle] {0} file(s)" -f $script:OfficeInputs.Count)
    $oracleInputList = Join-Path $oracleDir "inputs.json"
    @($script:OfficeInputs) | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $oracleInputList -Encoding UTF8
    $oracleArgs = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $oracle,
        "-RepoRoot",
        $root,
        "-InputListJson",
        $oracleInputList,
        "-OutputDir",
        $oracleDir,
        "-TimeoutSeconds",
        [string]$OfficeOracleTimeoutSeconds
    )
    if ($Visible) {
        $oracleArgs += "-Visible"
    }
    $oracleResult = Invoke-Process -FilePath "powershell.exe" -Arguments $oracleArgs
    if ($oracleResult.exitCode -ne 0) {
        Write-Warning ("Office oracle exited with code {0}. {1}" -f $oracleResult.exitCode, $oracleResult.output)
    }
    $officeResults = @()
    if (Test-Path -LiteralPath $oracleSummaryPath -PathType Leaf) {
        $officeResults = @(Get-Content -LiteralPath $oracleSummaryPath -Raw | ConvertFrom-Json)
    }
    $officeByPath = @{}
    foreach ($officeResult in $officeResults) {
        Add-OfficeResultToMap -Map $officeByPath -OfficeResult $officeResult
    }
    foreach ($scenario in $script:Scenarios) {
        if ($scenario.microsoftOffice.status -ne "pending") { continue }
        $scenarioKey = ([string]$scenario.file).Trim().ToLowerInvariant()
        $office = if ($officeByPath.ContainsKey($scenarioKey)) { $officeByPath[$scenarioKey] } else { $null }
        if ($null -eq $office) {
            $scenario.microsoftOffice = New-Stage -Status "missing" -Detail "Office oracle did not report this file."
            $scenario.proofLevel = "failed"
        }
        elseif ($office.status -eq "passed") {
            $scenario.microsoftOffice = New-Stage -Status "passed" -Detail ("{0} opened the file without repair/failure." -f $office.officeApplication) -Artifact $scenario.file -ElapsedMs $office.elapsedMs
            $scenario.proofLevel = "microsoft-office-com-open"
        }
        else {
            $scenario.microsoftOffice = New-Stage -Status "failed" -Detail $office.errorMessage -Artifact $scenario.file -ElapsedMs $office.elapsedMs
            $scenario.proofLevel = "failed"
        }
    }
    if (@($officeResults | Where-Object { $_.status -ne "passed" }).Count -gt 0) {
        Write-Warning "Office oracle reported at least one failure."
    }
}

$failed = @($script:Scenarios | Where-Object {
    $_.readback.status -eq "failed" -or
    $_.strict.status -eq "failed" -or
    $_.conformance.status -eq "failed" -or
    $_.openXmlSdk.status -eq "failed" -or
    $_.microsoftOffice.status -eq "failed" -or
    $_.microsoftOffice.status -eq "missing" -or
    ($_.PSObject.Properties.Name -contains "guard" -and $_.guard.status -eq "failed")
})
$proofLevel = if ($failed.Count -gt 0) {
    "failed"
}
elseif ($SkipOffice -and $script:RunOpenXmlSdk) {
    "openxml-sdk-schema"
}
elseif ($SkipOffice -and $RunConformance) {
    "repair-conformance"
}
elseif ($SkipOffice) {
    "strict-validation"
}
else {
    "microsoft-office-com-open"
}

$artifactProofMatrix = [pscustomobject]@{
    enabled              = [bool]($WriteArtifactProofMatrix -or $FailOnArtifactProofGap)
    status               = "not-run"
    json                 = ""
    markdown             = ""
    evidence             = ""
    command              = ""
    exitCode             = $null
    mutatingCommandCount = $null
    rowsWithRequiredGaps = $null
    gapsByTier           = $null
}

$summary = [pscustomobject]@{
    timestampUtc        = [DateTime]::UtcNow.ToString("o")
    repoRoot            = $root
    outputDir           = $outRoot
    binary              = $BinaryPath
    proofLevel          = $proofLevel
    openXmlSetup        = $openXmlSetup
    runConformance      = [bool]$RunConformance
    skipOffice          = [bool]$SkipOffice
    officeOracleSummary = if (Test-Path -LiteralPath $oracleSummaryPath -PathType Leaf) { $oracleSummaryPath } else { "" }
    scenarioCount       = $script:Scenarios.Count
    passedCount         = $script:Scenarios.Count - $failed.Count
    failedCount         = $failed.Count
    artifactProofMatrix = $artifactProofMatrix
    scenarios           = $script:Scenarios
}

$summaryPath = Join-Path $outRoot "summary.json"
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8

if ($WriteArtifactProofMatrix -or $FailOnArtifactProofGap) {
    $matrixScript = Join-Path $PSScriptRoot "artifact-proof-matrix.ps1"
    if (-not (Test-Path -LiteralPath $matrixScript -PathType Leaf)) {
        throw "artifact proof matrix script was not found: $matrixScript"
    }
    if ($ArtifactProofMatrixJson -eq "") {
        $ArtifactProofMatrixJson = Join-Path $outRoot "artifact-proof-matrix.json"
    }
    if ($ArtifactProofMatrixMarkdown -eq "") {
        $ArtifactProofMatrixMarkdown = Join-Path $outRoot "artifact-proof-matrix.md"
    }

    $evidencePath = Join-Path $outRoot "vba-artifact-proof-evidence.json"
    New-VbaProofEvidence -SummaryPath $summaryPath | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $evidencePath -Encoding UTF8
    $matrixArgs = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $matrixScript,
        "-RepoRoot",
        $root,
        "-BinaryPath",
        $BinaryPath,
        "-EvidencePath",
        $evidencePath,
        "-OutJson",
        $ArtifactProofMatrixJson,
        "-OutMarkdown",
        $ArtifactProofMatrixMarkdown
    )
    if ($OfficeEditSmokeSummaryPath -ne "") {
        $matrixArgs += "-OfficeEditSmokeSummaryPath"
        $matrixArgs += $OfficeEditSmokeSummaryPath
    }
    if ($FailOnArtifactProofGap) {
        $matrixArgs += "-FailOnGap"
    }

    $artifactProofMatrix.json = $ArtifactProofMatrixJson
    $artifactProofMatrix.markdown = $ArtifactProofMatrixMarkdown
    $artifactProofMatrix.evidence = $evidencePath
    $artifactProofMatrix.command = Format-CommandLine -FilePath "powershell.exe" -Arguments $matrixArgs

    Write-Host ("[artifact-proof-matrix] {0}" -f $artifactProofMatrix.command)
    & powershell.exe @matrixArgs
    $matrixExitCode = $LASTEXITCODE
    $artifactProofMatrix.exitCode = $matrixExitCode
    $artifactProofMatrix.status = if ($matrixExitCode -eq 0) { "written" } else { "failed" }
    if (Test-Path -LiteralPath $ArtifactProofMatrixJson -PathType Leaf) {
        try {
            $matrixDoc = Get-Content -LiteralPath $ArtifactProofMatrixJson -Raw | ConvertFrom-Json
            $artifactProofMatrix.mutatingCommandCount = [int]$matrixDoc.summary.mutatingCommandCount
            $artifactProofMatrix.rowsWithRequiredGaps = [int]$matrixDoc.summary.rowsWithRequiredGaps
            $artifactProofMatrix.gapsByTier = $matrixDoc.summary.gapsByTier
            if ($artifactProofMatrix.rowsWithRequiredGaps -gt 0) {
                $artifactProofMatrix.status = "gaps"
            }
            elseif ($matrixExitCode -eq 0) {
                $artifactProofMatrix.status = "covered"
            }
        }
        catch {
            if ($matrixExitCode -eq 0) {
                $artifactProofMatrix.status = "written"
            }
        }
    }
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    if ($matrixExitCode -ne 0) {
        Write-Host ("Summary: {0}" -f $summaryPath)
        throw ("artifact proof matrix failed with exit code {0}" -f $matrixExitCode)
    }
}

Write-Host ("Summary: {0}" -f $summaryPath)
Write-Host ("Result: {0} ({1}/{2} passed)" -f $proofLevel, $summary.passedCount, $summary.scenarioCount)
if ($artifactProofMatrix.enabled) {
    Write-Host ("Artifact proof matrix: {0}" -f $artifactProofMatrix.json)
}

if ($failed.Count -gt 0) {
    exit 1
}
