[CmdletBinding()]
param(
    [string]$RepoRoot = "",

    [string]$OutputDir = (Join-Path $env:TEMP ("ooxml-office-vba-run-smoke-" + [guid]::NewGuid().ToString("N"))),

    [string]$BinaryPath = "",

    [string]$InputFile = "",

    [string]$MacroName = "AgentSmokeRun",

    [string]$ExpectedCell = "A1",

    [string]$ExpectedValue = "Hello from ooxml",

    [string]$MarkerFile = "",

    [int]$TimeoutSeconds = 120,

    [switch]$Visible,

    [switch]$ChildRunOnly,

    [string]$ChildOutputJson = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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

function Resolve-SmokePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Root
    )
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $Root $Path))
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

function Resolve-DefaultBinaryPath {
    param([string]$Root)
    $metadata = Invoke-Process -FilePath "cargo" -Arguments @("metadata", "--format-version", "1", "--no-deps")
    if ($metadata.exitCode -eq 0 -and $metadata.output -ne "") {
        try {
            $parsed = $metadata.output | ConvertFrom-Json
            if ($parsed.target_directory -ne "") {
                return (Join-Path ([string]$parsed.target_directory) "debug\ooxml.exe")
            }
        }
        catch {}
    }
    return (Join-Path $Root "target\debug\ooxml.exe")
}

function Release-ComObject {
    param([object]$Object)
    if ($null -ne $Object -and [System.Runtime.InteropServices.Marshal]::IsComObject($Object)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Object)
    }
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

function Get-ProcessIdSet {
    $ids = @{}
    foreach ($process in @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue)) {
        $ids[[int]$process.Id] = $true
    }
    return $ids
}

function Stop-NewExcelProcesses {
    param(
        [hashtable]$ExistingIds,
        [datetime]$StartedAt
    )
    foreach ($process in @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue)) {
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

function Write-AsciiFile {
    param([string]$Path, [string[]]$Lines)
    ($Lines -join "`r`n") + "`r`n" | Set-Content -LiteralPath $Path -Encoding ASCII -NoNewline
}

function Escape-VbaString {
    param([string]$Value)
    return ($Value -replace '"', '""')
}

function New-SmokeResult {
    param(
        [string]$Status,
        [string]$File,
        [string]$Macro,
        [string]$ActualValue = "",
        [int64]$ElapsedMs = 0,
        [string]$OfficeVersion = "",
        [string]$OfficeBuild = "",
        [string]$ErrorType = "",
        [string]$ErrorMessage = ""
    )
    [pscustomobject]@{
        timestampUtc      = [DateTime]::UtcNow.ToString("o")
        file              = $File
        family            = "xlsx"
        officeApplication = "Excel"
        officeVersion     = $OfficeVersion
        officeBuild       = $OfficeBuild
        status            = $Status
        visible           = [bool]$Visible
        macroName         = $Macro
        expectedCell      = $ExpectedCell
        expectedValue     = $ExpectedValue
        actualValue       = $ActualValue
        markerFile        = $MarkerFile
        elapsedMs         = $ElapsedMs
        errorType         = $ErrorType
        errorMessage      = $ErrorMessage
    }
}

function Get-ObjectPropertyString {
    param(
        [object]$Object,
        [string]$Name
    )
    $candidate = $Object
    if ($candidate -is [System.Array]) {
        $candidate = @($candidate) | Select-Object -Last 1
    }
    if ($null -eq $candidate) {
        return ""
    }
    $property = @($candidate.PSObject.Properties | Where-Object { $_.Name -ieq $Name } | Select-Object -First 1)
    if ($property.Count -eq 0) {
        return ""
    }
    return [string]$property[0].Value
}

function Get-SmokeStatus {
    param([object]$Result)
    return (Get-ObjectPropertyString -Object $Result -Name "status")
}

function Write-ProgressEvent {
    param(
        [string]$Stage,
        [string]$Detail = ""
    )
    try {
        $path = Join-Path $OutputDir "child-progress.jsonl"
        ([pscustomobject]@{
            timestampUtc = [DateTime]::UtcNow.ToString("o")
            stage        = $Stage
            detail       = $Detail
        } | ConvertTo-Json -Compress) | Add-Content -LiteralPath $path -Encoding UTF8
    }
    catch {}
}

function Write-ChildResultBestEffort {
    param([object]$Result)
    if (-not $ChildRunOnly -or $ChildOutputJson -eq "") {
        return
    }
    try {
        $Result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ChildOutputJson -Encoding UTF8
    }
    catch {}
}

function Invoke-ExcelMacroRun {
    param([string]$Path)

    $excel = $null
    $workbook = $null
    $worksheet = $null
    $range = $null
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        Write-ProgressEvent -Stage "excel-create" -Detail "Creating Excel COM application."
        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = [bool]$Visible
        $excel.DisplayAlerts = $false
        try {
            # 1 is msoAutomationSecurityLow. This script is explicitly opt-in.
            $excel.AutomationSecurity = 1
        }
        catch {}

        Write-ProgressEvent -Stage "workbook-open" -Detail $Path
        $workbook = $excel.Workbooks.Open($Path, 0, $false)
        $macro = "'$($workbook.Name)'!$MacroName"
        Write-ProgressEvent -Stage "macro-run" -Detail $macro
        [void]$excel.Run($macro)
        if ($MarkerFile -ne "") {
            Write-ProgressEvent -Stage "read-marker" -Detail $MarkerFile
            $deadline = (Get-Date).AddSeconds(5)
            while (-not (Test-Path -LiteralPath $MarkerFile -PathType Leaf) -and (Get-Date) -lt $deadline) {
                Start-Sleep -Milliseconds 100
            }
            if (Test-Path -LiteralPath $MarkerFile -PathType Leaf) {
                $actual = (Get-Content -LiteralPath $MarkerFile -Raw).Trim()
            }
            else {
                $actual = ""
            }
        }
        else {
            Write-ProgressEvent -Stage "read-cell" -Detail $ExpectedCell
            $worksheet = $workbook.Worksheets.Item(1)
            $range = $worksheet.Range($ExpectedCell)
            $actual = [string]$range.Value2
        }
        Write-ProgressEvent -Stage "macro-result" -Detail $actual
        $timer.Stop()
        $status = if ($actual -eq $ExpectedValue) { "passed" } else { "failed" }
        $errorType = if ($status -eq "passed") { "" } else { "UnexpectedCellValue" }
        $errorMessage = if ($status -eq "passed") { "" } else { "Expected $ExpectedCell to equal '$ExpectedValue' after macro run, got '$actual'." }
        $result = New-SmokeResult -Status $status -File $Path -Macro $MacroName -ActualValue $actual -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion ([string]$excel.Version) -OfficeBuild ([string]$excel.Build) -ErrorType $errorType -ErrorMessage $errorMessage
        Write-ChildResultBestEffort -Result $result
        return $result
    }
    catch {
        $timer.Stop()
        $version = ""
        $build = ""
        try {
            if ($null -ne $excel) {
                $version = [string]$excel.Version
                $build = [string]$excel.Build
            }
        }
        catch {}
        $result = New-SmokeResult -Status "failed" -File $Path -Macro $MacroName -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $version -OfficeBuild $build -ErrorType $_.Exception.GetType().FullName -ErrorMessage $_.Exception.Message
        Write-ChildResultBestEffort -Result $result
        return $result
    }
    finally {
        if ($null -ne $workbook) {
            try { [void]$workbook.Close($false) } catch {}
        }
        if ($null -ne $excel) {
            try { [void]$excel.Quit() } catch {}
        }
        Release-ComObject -Object $range
        Release-ComObject -Object $worksheet
        Release-ComObject -Object $workbook
        Release-ComObject -Object $excel
    }
}

function Invoke-ChildMacroRun {
    param([string]$Path)

    $childJson = Join-Path $OutputDir "child-result.json"
    $childStdout = Join-Path $OutputDir "child.stdout.txt"
    $childStderr = Join-Path $OutputDir "child.stderr.txt"
    Remove-Item -LiteralPath $childJson, $childStdout, $childStderr -Force -ErrorAction SilentlyContinue

    $excelIdsBefore = Get-ProcessIdSet
    $startedAt = Get-Date
    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", $PSCommandPath,
        "-RepoRoot", $RepoRoot,
        "-OutputDir", $OutputDir,
        "-InputFile", $Path,
        "-MacroName", $MacroName,
        "-ExpectedCell", $ExpectedCell,
        "-ExpectedValue", $ExpectedValue,
        "-MarkerFile", $MarkerFile,
        "-TimeoutSeconds", "0",
        "-ChildRunOnly",
        "-ChildOutputJson", $childJson
    )
    if ($Visible) {
        $arguments += "-Visible"
    }
    $argumentLine = ($arguments | ForEach-Object { Quote-Argument -Value $_ }) -join " "
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath "powershell.exe" -ArgumentList $argumentLine -WorkingDirectory $RepoRoot -RedirectStandardOutput $childStdout -RedirectStandardError $childStderr -WindowStyle Hidden -PassThru
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    $timer.Stop()

    if (-not $finished) {
        Stop-ProcessTree -ProcessId $process.Id
        Stop-NewExcelProcesses -ExistingIds $excelIdsBefore -StartedAt $startedAt
        if (Test-Path -LiteralPath $childJson -PathType Leaf) {
            return (Get-Content -LiteralPath $childJson -Raw | ConvertFrom-Json)
        }
        $output = ((Read-TextFileBestEffort -Path $childStdout), (Read-TextFileBestEffort -Path $childStderr) | Where-Object { $null -ne $_ -and [string]$_ -ne "" }) -join [Environment]::NewLine
        return (New-SmokeResult -Status "failed" -File $Path -Macro $MacroName -ElapsedMs $timer.ElapsedMilliseconds -ErrorType "Timeout" -ErrorMessage ("Macro run exceeded {0} second(s). {1}" -f $TimeoutSeconds, $output.Trim()))
    }

    try {
        $process.WaitForExit()
        $process.Refresh()
    }
    catch {}
    $childResult = $null
    if (Test-Path -LiteralPath $childJson -PathType Leaf) {
        $childResult = Get-Content -LiteralPath $childJson -Raw | ConvertFrom-Json
    }

    $exitCode = $process.ExitCode
    if ($null -eq $exitCode -and $null -ne $childResult) {
        $exitCode = if ((Get-SmokeStatus -Result $childResult) -eq "passed") { 0 } else { 1 }
    }
    if ($null -eq $exitCode) {
        $exitCode = 1
    }

    if ($exitCode -ne 0) {
        Stop-NewExcelProcesses -ExistingIds $excelIdsBefore -StartedAt $startedAt
        if ($null -ne $childResult) {
            return $childResult
        }
        $output = ((Read-TextFileBestEffort -Path $childStdout), (Read-TextFileBestEffort -Path $childStderr) | Where-Object { $null -ne $_ -and [string]$_ -ne "" }) -join [Environment]::NewLine
        return (New-SmokeResult -Status "failed" -File $Path -Macro $MacroName -ElapsedMs $timer.ElapsedMilliseconds -ErrorType "ChildProcessFailed" -ErrorMessage ("Child macro runner exited with {0}. {1}" -f $exitCode, $output.Trim()))
    }
    if ($null -eq $childResult) {
        return (New-SmokeResult -Status "failed" -File $Path -Macro $MacroName -ElapsedMs $timer.ElapsedMilliseconds -ErrorType "MissingChildResult" -ErrorMessage "Child macro runner did not write its result JSON.")
    }
    return $childResult
}

if ($RepoRoot.Trim() -eq "") {
    $scriptRoot = $PSScriptRoot
    if ($scriptRoot -eq "") {
        $scriptRoot = Split-Path -Parent $PSCommandPath
    }
    $RepoRoot = (Resolve-Path -LiteralPath (Join-Path $scriptRoot "..")).Path
}
$root = (Resolve-Path -LiteralPath $RepoRoot).Path
$outRoot = [System.IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $outRoot | Out-Null

if ($ChildRunOnly) {
    if ($InputFile -eq "") {
        throw "-InputFile is required with -ChildRunOnly."
    }
    $result = Invoke-ExcelMacroRun -Path ([System.IO.Path]::GetFullPath($InputFile))
    if ($ChildOutputJson -ne "") {
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ChildOutputJson -Encoding UTF8
    }
    else {
        $result | ConvertTo-Json -Depth 6
    }
    if ((Get-SmokeStatus -Result $result) -ne "passed") {
        exit 1
    }
    exit 0
}

if ($InputFile -eq "") {
    if ($BinaryPath -eq "") {
        $BinaryPath = Resolve-DefaultBinaryPath -Root $root
    }
    if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
        throw "BinaryPath does not exist: $BinaryPath. Run `cargo build --bin ooxml` or pass -BinaryPath."
    }
    $sourcePath = Join-Path $outRoot "AgentSmoke.bas"
    $xlsxPath = Join-Path $outRoot "input.xlsx"
    $xlsmPath = Join-Path $outRoot "macro-run-smoke.xlsm"
    if ($MarkerFile -eq "") {
        $MarkerFile = Join-Path $outRoot "macro-run-marker.txt"
    }
    Remove-Item -LiteralPath $MarkerFile -Force -ErrorAction SilentlyContinue
    Write-AsciiFile -Path $sourcePath -Lines @(
        'Attribute VB_Name = "AgentSmoke"',
        'Public Sub AgentSmokeRun()',
        ('    ThisWorkbook.Worksheets(1).Range("{0}").Value = "{1}"' -f $ExpectedCell, ($ExpectedValue -replace '"', '""')),
        ('    Open "{0}" For Output As #1' -f (Escape-VbaString -Value $MarkerFile)),
        ('    Print #1, "{0}"' -f (Escape-VbaString -Value $ExpectedValue)),
        '    Close #1',
        'End Sub'
    )
    Copy-Item -LiteralPath (Join-Path $root "testdata\xlsx\minimal-workbook\workbook.xlsx") -Destination $xlsxPath -Force
    $create = Invoke-Process -FilePath $BinaryPath -Arguments @("--json", "vba", "create", $xlsxPath, "--pure", "--source", $sourcePath, "--out", $xlsmPath)
    if ($create.exitCode -ne 0) {
        throw ("pure XLSM create failed: {0}" -f $create.output)
    }
    $validate = Invoke-Process -FilePath $BinaryPath -Arguments @("--json", "validate", "--strict", $xlsmPath)
    if ($validate.exitCode -ne 0) {
        throw ("strict validation failed before macro run: {0}" -f $validate.output)
    }
    $InputFile = $xlsmPath
}
else {
    $InputFile = Resolve-SmokePath -Path $InputFile -Root $root
}

$summaryPath = Join-Path $outRoot "summary.json"
$result = Invoke-ChildMacroRun -Path $InputFile
$summary = [pscustomobject]@{
    schemaVersion = "ooxml-cli.vba-run-smoke.v1"
    repoRoot      = $root
    outputDir     = $outRoot
    inputFile     = $InputFile
    result        = $result
}
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8

Write-Host ("Summary: {0}" -f $summaryPath)
Write-Host ("Result: {0}" -f (Get-SmokeStatus -Result $result))
if ((Get-SmokeStatus -Result $result) -ne "passed") {
    exit 1
}
