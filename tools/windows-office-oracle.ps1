[CmdletBinding()]
param(
    [Alias("File", "Path")]
    [string[]]$InputFile = @(),

    [string]$RepoRoot = (Get-Location).Path,

    [string]$OutputDir = "office-oracle-proof",

    [int]$TimeoutSeconds = 120,

    [switch]$ChildOpenOnly,

    [string]$ChildOutputJson = "",

    [switch]$Visible
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-OraclePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Root
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return [System.IO.Path]::GetFullPath((Join-Path $Root $Path))
}

function Release-ComObject {
    param([object]$Object)

    if ($null -ne $Object -and [System.Runtime.InteropServices.Marshal]::IsComObject($Object)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Object)
    }
}

function New-OracleResult {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Family,

        [Parameter(Mandatory = $true)]
        [string]$Application,

        [Parameter(Mandatory = $true)]
        [string]$Status,

        [int64]$ElapsedMs = 0,

        [string]$OfficeVersion = "",

        [string]$OfficeBuild = "",

        [string]$ErrorType = "",

        [string]$ErrorMessage = ""
    )

    [pscustomobject]@{
        timestampUtc      = [DateTime]::UtcNow.ToString("o")
        file              = $Path
        family            = $Family
        officeApplication = $Application
        officeVersion     = $OfficeVersion
        officeBuild       = $OfficeBuild
        status            = $Status
        visible           = [bool]$Visible
        elapsedMs         = $ElapsedMs
        errorType         = $ErrorType
        errorMessage      = $ErrorMessage
    }
}

function Get-OracleFamily {
    param([string]$Path)

    switch ([System.IO.Path]::GetExtension($Path).ToLowerInvariant()) {
        ".xlsx" { return "xlsx" }
        ".xlsm" { return "xlsx" }
        ".pptx" { return "pptx" }
        ".pptm" { return "pptx" }
        ".docx" { return "docx" }
        ".docm" { return "docx" }
        default { return "unknown" }
    }
}

function Get-OracleApplication {
    param([string]$Family)

    switch ($Family) {
        "xlsx" { return "Excel" }
        "pptx" { return "PowerPoint" }
        "docx" { return "Word" }
        default { return "none" }
    }
}

function Invoke-OfficeOpenForPath {
    param([string]$Path)

    switch ([System.IO.Path]::GetExtension($Path).ToLowerInvariant()) {
        ".xlsx" { return (Test-ExcelOpen -Path $Path) }
        ".xlsm" { return (Test-ExcelOpen -Path $Path) }
        ".pptx" { return (Test-PowerPointOpen -Path $Path) }
        ".pptm" { return (Test-PowerPointOpen -Path $Path) }
        ".docx" { return (Test-WordOpen -Path $Path) }
        ".docm" { return (Test-WordOpen -Path $Path) }
        default {
            return (New-OracleResult -Path $Path -Family "unknown" -Application "none" -Status "failed" -ErrorType "UnsupportedExtension" -ErrorMessage "Supported extensions: .xlsx, .xlsm, .pptx, .pptm, .docx, .docm.")
        }
    }
}

function Get-OfficeProcessNamesForFamily {
    param([string]$Family)

    switch ($Family) {
        "xlsx" { return @("EXCEL") }
        "pptx" { return @("POWERPNT") }
        "docx" { return @("WINWORD") }
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

function Invoke-OfficeOpenWithTimeout {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Root,

        [Parameter(Mandatory = $true)]
        [string]$OutputDir,

        [Parameter(Mandatory = $true)]
        [int]$TimeoutSeconds,

        [Parameter(Mandatory = $true)]
        [int]$Index
    )

    if ($TimeoutSeconds -le 0) {
        return (Invoke-OfficeOpenForPath -Path $Path)
    }

    $family = Get-OracleFamily -Path $Path
    $application = Get-OracleApplication -Family $family
    $officeProcessNames = @(Get-OfficeProcessNamesForFamily -Family $family)
    $officeProcessIdsBefore = Get-ProcessIdSet -Names $officeProcessNames
    $childJson = Join-Path $OutputDir ("child-{0}.json" -f $Index)
    $childStdout = Join-Path $OutputDir ("child-{0}.stdout.txt" -f $Index)
    $childStderr = Join-Path $OutputDir ("child-{0}.stderr.txt" -f $Index)
    Remove-Item -LiteralPath $childJson, $childStdout, $childStderr -Force -ErrorAction SilentlyContinue

    function Quote-OracleArgument {
        param([string]$Value)

        if ($Value -eq "") {
            return '""'
        }
        if ($Value -match '[\s"]') {
            return '"' + ($Value -replace '"', '\"') + '"'
        }
        return $Value
    }

    $args = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", $PSCommandPath,
        "-InputFile", $Path,
        "-RepoRoot", $Root,
        "-OutputDir", $OutputDir,
        "-TimeoutSeconds", "0",
        "-ChildOpenOnly",
        "-ChildOutputJson", $childJson
    )
    if ($Visible) {
        $args += "-Visible"
    }

    $startedAt = Get-Date
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $argumentLine = ($args | ForEach-Object { Quote-OracleArgument -Value $_ }) -join " "
    $process = Start-Process -FilePath powershell.exe -ArgumentList $argumentLine -WorkingDirectory $Root -RedirectStandardOutput $childStdout -RedirectStandardError $childStderr -WindowStyle Hidden -PassThru
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    $timer.Stop()

    if (-not $finished) {
        try { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue } catch {}
        Stop-NewOfficeProcesses -Names $officeProcessNames -ExistingIds $officeProcessIdsBefore -StartedAt $startedAt
        return (New-OracleResult -Path $Path -Family $family -Application $application -Status "timeout" -ElapsedMs $timer.ElapsedMilliseconds -ErrorType "Timeout" -ErrorMessage ("Office COM open exceeded {0} second(s)." -f $TimeoutSeconds))
    }

    if (Test-Path -LiteralPath $childJson -PathType Leaf) {
        return (Get-Content -LiteralPath $childJson -Raw | ConvertFrom-Json)
    }

    $stderr = ""
    if (Test-Path -LiteralPath $childStderr -PathType Leaf) {
        $stderr = (Get-Content -LiteralPath $childStderr -Raw).Trim()
    }
    $stdout = ""
    if (Test-Path -LiteralPath $childStdout -PathType Leaf) {
        $stdout = (Get-Content -LiteralPath $childStdout -Raw).Trim()
    }
    $message = $stderr
    if ($message -eq "") {
        $message = $stdout
    }
    if ($message -eq "") {
        $message = ("Office oracle child exited with code {0} without writing a result." -f $process.ExitCode)
    }
    Stop-NewOfficeProcesses -Names $officeProcessNames -ExistingIds $officeProcessIdsBefore -StartedAt $startedAt
    return (New-OracleResult -Path $Path -Family $family -Application $application -Status "failed" -ElapsedMs $timer.ElapsedMilliseconds -ErrorType "ChildProcessFailed" -ErrorMessage $message)
}

function Get-OfficeIdentity {
    param([object]$Application)

    $version = ""
    $build = ""
    if ($null -ne $Application) {
        try { $version = [string]$Application.Version } catch {}
        try { $build = [string]$Application.Build } catch {}
    }

    [pscustomobject]@{
        version = $version
        build   = $build
    }
}

function Set-AutomationSecurity {
    param([object]$Application)

    try {
        # 3 is msoAutomationSecurityForceDisable.
        $Application.AutomationSecurity = 3
    }
    catch {
        # Some Office builds do not expose this property on every application.
    }
}

function Test-ExcelOpen {
    param([string]$Path)

    $excel = $null
    $workbook = $null
    $identity = [pscustomobject]@{ version = ""; build = "" }
    $timer = [System.Diagnostics.Stopwatch]::StartNew()

    try {
        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = [bool]$Visible
        $excel.DisplayAlerts = $false
        Set-AutomationSecurity -Application $excel
        $identity = Get-OfficeIdentity -Application $excel

        $workbook = $excel.Workbooks.Open($Path, 0, $true)
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "xlsx" -Application "Excel" -Status "passed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build
    }
    catch {
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "xlsx" -Application "Excel" -Status "failed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build -ErrorType $_.Exception.GetType().FullName -ErrorMessage $_.Exception.Message
    }
    finally {
        if ($null -ne $workbook) {
            try { $workbook.Close($false) } catch {}
        }
        if ($null -ne $excel) {
            try { $excel.Quit() } catch {}
        }
        Release-ComObject -Object $workbook
        Release-ComObject -Object $excel
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function Test-PowerPointOpen {
    param([string]$Path)

    $powerpoint = $null
    $presentation = $null
    $identity = [pscustomobject]@{ version = ""; build = "" }
    $timer = [System.Diagnostics.Stopwatch]::StartNew()

    try {
        $powerpoint = New-Object -ComObject PowerPoint.Application
        try {
            # 1 is ppAlertsNone. Keep PowerPoint aligned with the Word/Excel
            # oracle paths so modal repair or recovery prompts become failures
            # through the bounded child timeout instead of hanging the run.
            $powerpoint.DisplayAlerts = 1
        }
        catch {
            # Some Office builds may not expose this property through COM.
        }
        Set-AutomationSecurity -Application $powerpoint
        $identity = Get-OfficeIdentity -Application $powerpoint

        $presentation = $powerpoint.Presentations.Open($Path, $true, $false, [bool]$Visible)
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "pptx" -Application "PowerPoint" -Status "passed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build
    }
    catch {
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "pptx" -Application "PowerPoint" -Status "failed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build -ErrorType $_.Exception.GetType().FullName -ErrorMessage $_.Exception.Message
    }
    finally {
        if ($null -ne $presentation) {
            try { $presentation.Close() } catch {}
        }
        if ($null -ne $powerpoint) {
            try { $powerpoint.Quit() } catch {}
        }
        Release-ComObject -Object $presentation
        Release-ComObject -Object $powerpoint
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function Test-WordOpen {
    param([string]$Path)

    $word = $null
    $document = $null
    $identity = [pscustomobject]@{ version = ""; build = "" }
    $timer = [System.Diagnostics.Stopwatch]::StartNew()

    try {
        $word = New-Object -ComObject Word.Application
        $word.Visible = [bool]$Visible
        $word.DisplayAlerts = 0
        Set-AutomationSecurity -Application $word
        $identity = Get-OfficeIdentity -Application $word

        $document = $word.Documents.Open($Path, $false, $true)
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "docx" -Application "Word" -Status "passed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build
    }
    catch {
        $timer.Stop()
        return New-OracleResult -Path $Path -Family "docx" -Application "Word" -Status "failed" -ElapsedMs $timer.ElapsedMilliseconds -OfficeVersion $identity.version -OfficeBuild $identity.build -ErrorType $_.Exception.GetType().FullName -ErrorMessage $_.Exception.Message
    }
    finally {
        if ($null -ne $document) {
            try { $document.Close($false) } catch {}
        }
        if ($null -ne $word) {
            try { $word.Quit() } catch {}
        }
        Release-ComObject -Object $document
        Release-ComObject -Object $word
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

$root = (Resolve-Path -LiteralPath $RepoRoot).Path
$output = Resolve-OraclePath -Path $OutputDir -Root $root
New-Item -ItemType Directory -Force -Path $output | Out-Null

if ($InputFile.Count -eq 0) {
    throw "Provide one or more files with -InputFile."
}

if ($ChildOpenOnly) {
    $path = Resolve-OraclePath -Path $InputFile[0] -Root $root
    $result = $null
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        $result = New-OracleResult -Path $path -Family "unknown" -Application "none" -Status "failed" -ErrorType "FileNotFound" -ErrorMessage "File does not exist."
    }
    else {
        $result = Invoke-OfficeOpenForPath -Path $path
    }
    if ($ChildOutputJson -ne "") {
        $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ChildOutputJson -Encoding UTF8
    }
    else {
        $result | ConvertTo-Json -Depth 5
    }
    if ($result.status -eq "passed") {
        exit 0
    }
    exit 1
}

$results = New-Object System.Collections.Generic.List[object]
$inputIndex = 0

foreach ($input in $InputFile) {
    $path = Resolve-OraclePath -Path $input -Root $root

    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        $results.Add((New-OracleResult -Path $path -Family "unknown" -Application "none" -Status "failed" -ErrorType "FileNotFound" -ErrorMessage "File does not exist."))
        continue
    }

    $results.Add((Invoke-OfficeOpenWithTimeout -Path $path -Root $root -OutputDir $output -TimeoutSeconds $TimeoutSeconds -Index $inputIndex))
    $inputIndex++
}

$summaryPath = Join-Path $output "summary.json"
$jsonlPath = Join-Path $output "results.jsonl"

$results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
Remove-Item -LiteralPath $jsonlPath -Force -ErrorAction SilentlyContinue
foreach ($result in $results) {
    $result | ConvertTo-Json -Compress -Depth 5 | Add-Content -LiteralPath $jsonlPath -Encoding UTF8
}

$failed = @($results | Where-Object { $_.status -ne "passed" })
Write-Host ("Office oracle checked {0} file(s); {1} passed, {2} failed." -f $results.Count, ($results.Count - $failed.Count), $failed.Count)
Write-Host ("Summary: {0}" -f $summaryPath)

if ($failed.Count -gt 0) {
    exit 1
}
