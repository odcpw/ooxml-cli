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

    [switch]$SkipOffice,

    [switch]$EnableVbaObjectModelAccess,

    [switch]$Visible
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

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments, [string]$Label)
    Write-Host ("[{0}] {1}" -f $Label, (Format-CommandLine -FilePath $FilePath -Arguments $Arguments))
    $result = Invoke-Process -FilePath $FilePath -Arguments $Arguments
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

function Add-FileScenario {
    param([string]$Name, [string]$Family, [string]$Path)
    $strict = Invoke-StrictValidation -Path $Path
    $openXml = Invoke-OpenXmlValidation -Path $Path
    $script:Scenarios.Add([pscustomobject]@{
        name            = $Name
        family          = $Family
        file            = $Path
        proofLevel      = "pending-office"
        strict          = $strict
        openXmlSdk      = $openXml
        microsoftOffice = (New-Stage -Status "pending" -Detail "Waiting for desktop Office COM oracle.")
    })
    if ($strict.status -eq "passed" -and $openXml.status -ne "failed") {
        $script:OfficeInputs.Add($Path)
    }
}

function Add-GuardScenario {
    param([string]$Name, [string]$Family, [string[]]$Arguments, [string]$OutputPath)
    $result = Invoke-Process -FilePath $BinaryPath -Arguments $Arguments
    $passed = $result.exitCode -ne 0 -and
        $result.output -match "version-dependent _VBA_PROJECT metadata" -and
        (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf))
    $script:Scenarios.Add([pscustomobject]@{
        name            = $Name
        family          = $Family
        file            = $OutputPath
        proofLevel      = if ($passed) { "guarded-refusal" } else { "failed" }
        strict          = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        openXmlSdk      = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        microsoftOffice = (New-Stage -Status "not-run" -Detail "Guard scenario does not write an output file.")
        guard           = if ($passed) {
            New-Stage -Status "passed" -Detail "Command refused an Office-shaped module-set change before writing output." -Command $result.command -ElapsedMs $result.elapsedMs
        } else {
            New-Stage -Status "failed" -Detail ("Expected guarded refusal, got exit {0}: {1}" -f $result.exitCode, $result.output) -Command $result.command -ElapsedMs $result.elapsedMs
        }
    })
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
New-Item -ItemType Directory -Force -Path $outRoot, $binDir, $seedDir, $sourceDir, $caseDir, $oracleDir | Out-Null

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
Invoke-Checked -FilePath $BinaryPath -Arguments ($seedPrefixArgs + @($xlsmSeed, "--family", "xlsx") + $seedTailArgs) -Label "seed:xlsm:create" | Out-Null
Invoke-Checked -FilePath $BinaryPath -Arguments ($seedPrefixArgs + @($pptmSeed, "--family", "pptx") + $seedTailArgs) -Label "seed:pptm:create" | Out-Null

$script:Scenarios = New-Object System.Collections.Generic.List[object]
$script:OfficeInputs = New-Object System.Collections.Generic.List[string]

$families = @(
    [pscustomobject]@{
        family = "xlsx"; macroFamily = "xlsm"; base = (Join-Path $root "testdata\xlsx\minimal-workbook\workbook.xlsx"); seed = $xlsmSeed; attached = (Join-Path $caseDir "attached.xlsm"); removed = (Join-Path $caseDir "removed.xlsx"); replaced = (Join-Path $caseDir "replaced.xlsm"); addBlocked = (Join-Path $caseDir "add-blocked.xlsm"); removeBlocked = (Join-Path $caseDir "remove-blocked.xlsm")
    },
    [pscustomobject]@{
        family = "pptx"; macroFamily = "pptm"; base = (Join-Path $root "testdata\pptx\minimal-title\presentation.pptx"); seed = $pptmSeed; attached = (Join-Path $caseDir "attached.pptm"); removed = (Join-Path $caseDir "removed.pptx"); replaced = (Join-Path $caseDir "replaced.pptm"); addBlocked = (Join-Path $caseDir "add-blocked.pptm"); removeBlocked = (Join-Path $caseDir "remove-blocked.pptm")
    }
)

foreach ($item in $families) {
    Add-FileScenario -Name ("vba-{0}-office-seed" -f $item.macroFamily) -Family $item.family -Path $item.seed

    $binPath = Join-Path $caseDir ("{0}-vbaProject.bin" -f $item.macroFamily)
    Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "extract-bin", $item.seed, "--out", $binPath) -Label ("vba:{0}:extract-bin" -f $item.macroFamily) | Out-Null
    Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "attach", $item.base, "--bin", $binPath, "--out", $item.attached) -Label ("vba:{0}:attach" -f $item.macroFamily) | Out-Null
    Add-FileScenario -Name ("vba-{0}-attach-office-bin" -f $item.macroFamily) -Family $item.family -Path $item.attached

    Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "remove", $item.attached, "--out", $item.removed) -Label ("vba:{0}:remove-project" -f $item.macroFamily) | Out-Null
    Add-FileScenario -Name ("vba-{0}-remove-project" -f $item.macroFamily) -Family $item.family -Path $item.removed

    $list = (& $BinaryPath --format json vba list $item.seed | ConvertFrom-Json)
    $standard = $list.project.modules | Where-Object { $_.name -eq "SeedModule" } | Select-Object -First 1
    $classModule = $list.project.modules | Where-Object { $_.name -eq "SeedClass" } | Select-Object -First 1
    if ($null -eq $standard -or $null -eq $classModule) {
        throw ("{0} seed did not expose SeedModule and SeedClass through vba list." -f $item.macroFamily)
    }
    Invoke-Checked -FilePath $BinaryPath -Arguments @("--format", "json", "vba", "replace-module", $item.seed, "--module", $standard.primarySelector, "--source", $replacementSource, "--expect-sha256", $standard.sha256, "--allow-experimental-vba-source-rewrite", "--out", $item.replaced) -Label ("vba:{0}:replace-module" -f $item.macroFamily) | Out-Null
    Add-FileScenario -Name ("vba-{0}-replace-existing-bas" -f $item.macroFamily) -Family $item.family -Path $item.replaced

    Add-GuardScenario -Name ("vba-{0}-add-module-guard" -f $item.macroFamily) -Family $item.family -Arguments @("--format", "json", "vba", "add-module", $item.seed, "--source", $agentSource, "--allow-experimental-vba-source-rewrite", "--out", $item.addBlocked) -OutputPath $item.addBlocked
    Add-GuardScenario -Name ("vba-{0}-remove-module-guard" -f $item.macroFamily) -Family $item.family -Arguments @("--format", "json", "vba", "remove-module", $item.seed, "--module", $classModule.primarySelector, "--expect-sha256", $classModule.sha256, "--allow-experimental-vba-source-rewrite", "--out", $item.removeBlocked) -OutputPath $item.removeBlocked
}

$oracleSummaryPath = Join-Path $oracleDir "summary.json"
if ($SkipOffice) {
    foreach ($scenario in $script:Scenarios) {
        if ($scenario.microsoftOffice.status -eq "pending") {
            $scenario.microsoftOffice = New-Stage -Status "skipped" -Detail "Skipped by -SkipOffice."
            if ($scenario.openXmlSdk.status -eq "passed") { $scenario.proofLevel = "openxml-sdk-schema" }
            elseif ($scenario.strict.status -eq "passed") { $scenario.proofLevel = "strict-validation" }
        }
    }
}
elseif ($script:OfficeInputs.Count -gt 0) {
    $oracle = Join-Path $root "tools\windows-office-oracle.ps1"
    Write-Host ("[office-oracle] {0} file(s)" -f $script:OfficeInputs.Count)
    if ($Visible) {
        & $oracle -RepoRoot $root -InputFile @($script:OfficeInputs) -OutputDir $oracleDir -TimeoutSeconds $OfficeOracleTimeoutSeconds -Visible
    }
    else {
        & $oracle -RepoRoot $root -InputFile @($script:OfficeInputs) -OutputDir $oracleDir -TimeoutSeconds $OfficeOracleTimeoutSeconds
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
    $_.strict.status -eq "failed" -or
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
elseif ($SkipOffice) {
    "strict-validation"
}
else {
    "microsoft-office-com-open"
}

$summary = [pscustomobject]@{
    timestampUtc        = [DateTime]::UtcNow.ToString("o")
    repoRoot            = $root
    outputDir           = $outRoot
    binary              = $BinaryPath
    proofLevel          = $proofLevel
    openXmlSetup        = $openXmlSetup
    skipOffice          = [bool]$SkipOffice
    officeOracleSummary = if (Test-Path -LiteralPath $oracleSummaryPath -PathType Leaf) { $oracleSummaryPath } else { "" }
    scenarioCount       = $script:Scenarios.Count
    passedCount         = $script:Scenarios.Count - $failed.Count
    failedCount         = $failed.Count
    scenarios           = $script:Scenarios
}

$summaryPath = Join-Path $outRoot "summary.json"
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
Write-Host ("Summary: {0}" -f $summaryPath)
Write-Host ("Result: {0} ({1}/{2} passed)" -f $proofLevel, $summary.passedCount, $summary.scenarioCount)

if ($failed.Count -gt 0) {
    exit 1
}
