[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path,

    [string]$OutputDir = "",

    [string]$BinaryPath = "",

    [string]$GoExe = "",

    [string]$GoCache = "",

    [string]$DotNetExe = "",

    [string]$OpenXmlValidatorProject = "",

    [int]$MutationParallelism = ([Math]::Max(1, [Math]::Min(4, [Environment]::ProcessorCount))),

    [string[]]$ScenarioName = @(),

    [int]$OfficeOracleTimeoutSeconds = 120,

    [switch]$SkipBuild,

    [switch]$SkipOpenXmlSdk,

    [switch]$RequireOpenXmlSdk,

    [switch]$RunConformance,

    [switch]$SkipOffice,

    [switch]$Visible,

    [switch]$WriteArtifactProofMatrix,

    [switch]$FailOnArtifactProofGap,

    [string]$ArtifactProofMatrixJson = "",

    [string]$ArtifactProofMatrixMarkdown = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-GoExe {
    param([string]$Requested)

    if ($Requested -ne "") {
        return $Requested
    }

    $fromPath = Get-Command go -ErrorAction SilentlyContinue
    if ($null -ne $fromPath) {
        return $fromPath.Source
    }

    $default = "C:\Program Files\Go\bin\go.exe"
    if (Test-Path -LiteralPath $default -PathType Leaf) {
        return $default
    }

    throw "Go executable not found. Pass -GoExe or install Go."
}

function Resolve-DotNetExe {
    param([string]$Requested)

    if ($Requested -ne "") {
        return $Requested
    }

    $fromPath = Get-Command dotnet -ErrorAction SilentlyContinue
    if ($null -ne $fromPath) {
        return $fromPath.Source
    }

    $default = "C:\Program Files\dotnet\dotnet.exe"
    if (Test-Path -LiteralPath $default -PathType Leaf) {
        return $default
    }

    return ""
}

function Quote-Argument {
    param([string]$Value)

    if ($Value -eq "") {
        return '""'
    }
    if ($Value -match '[\s"]') {
        return '"' + ($Value -replace '"', '\"') + '"'
    }
    return $Value
}

function Format-CommandLine {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    return ((@($FilePath) + $Arguments) | ForEach-Object { Quote-Argument -Value $_ }) -join " "
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    Write-Host ("[{0}] {1}" -f $Label, (Format-CommandLine -FilePath $FilePath -Arguments $Arguments))
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw ("{0} failed with exit code {1}" -f $Label, $LASTEXITCODE)
    }
}

function Get-DocxTableHash {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryPath,

        [Parameter(Mandatory = $true)]
        [string]$DocumentPath,

        [int]$Table = 1
    )

    $args = @("--json", "docx", "tables", "show", $DocumentPath, "--table", ([string]$Table))
    $output = @(& $BinaryPath @args 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw ("stage:docx-table-hash failed with exit code {0}. {1}" -f $LASTEXITCODE, (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine))
    }

    $json = ($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
    $report = $json | ConvertFrom-Json
    $tables = @($report.tables)
    if ($tables.Count -ne 1) {
        throw ("stage:docx-table-hash expected one table for selector {0}, found {1}." -f $Table, $tables.Count)
    }
    if ($tables[0].merged -or $tables[0].rows -lt 2 -or $tables[0].cols -lt 2) {
        throw ("stage:docx-table-hash expected table {0} to be unmerged with at least 2 rows and 2 columns." -f $Table)
    }
    if ($tables[0].contentHash -eq $null -or $tables[0].contentHash -notmatch '^sha256:[0-9a-f]{64}$') {
        throw ("stage:docx-table-hash found table {0} with invalid contentHash: {1}" -f $Table, $tables[0].contentHash)
    }
    return [string]$tables[0].contentHash
}

function Get-DocxBlockHash {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryPath,

        [Parameter(Mandatory = $true)]
        [string]$DocumentPath,

        [int]$Block = 1
    )

    $args = @("--json", "docx", "blocks", $DocumentPath, "--block", ([string]$Block))
    $output = @(& $BinaryPath @args 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw ("stage:docx-block-hash failed with exit code {0}. {1}" -f $LASTEXITCODE, (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine))
    }

    $json = ($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
    $report = $json | ConvertFrom-Json
    $blocks = @($report.blocks)
    if ($blocks.Count -ne 1) {
        throw ("stage:docx-block-hash expected one block for selector {0}, found {1}." -f $Block, $blocks.Count)
    }
    if ($blocks[0].contentHash -eq $null -or $blocks[0].contentHash -notmatch '^sha256:[0-9a-f]{64}$') {
        throw ("stage:docx-block-hash found block {0} with invalid contentHash: {1}" -f $Block, $blocks[0].contentHash)
    }
    return [string]$blocks[0].contentHash
}

function Get-DocxCommentHash {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryPath,

        [Parameter(Mandatory = $true)]
        [string]$DocumentPath,

        [int]$CommentID = 0
    )

    $args = @("--json", "docx", "comments", "list", $DocumentPath, "--comment-id", ([string]$CommentID))
    $output = @(& $BinaryPath @args 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw ("stage:docx-comment-hash failed with exit code {0}. {1}" -f $LASTEXITCODE, (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine))
    }

    $json = ($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
    $report = $json | ConvertFrom-Json
    $comments = @($report.comments)
    if ($comments.Count -ne 1) {
        throw ("stage:docx-comment-hash expected one comment for id {0}, found {1}." -f $CommentID, $comments.Count)
    }
    if ($comments[0].contentHash -eq $null -or $comments[0].contentHash -notmatch '^sha256:[0-9a-f]{64}$') {
        throw ("stage:docx-comment-hash found comment {0} with invalid contentHash: {1}" -f $CommentID, $comments[0].contentHash)
    }
    return [string]$comments[0].contentHash
}

function New-SmokeWavFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $sampleRate = 8000
    $durationMs = 200
    $samples = [int]($sampleRate * $durationMs / 1000)
    $dataBytes = $samples * 2
    $riffSize = 36 + $dataBytes
    $writer = [System.IO.BinaryWriter]::new([System.IO.File]::Create($Path))
    try {
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("RIFF"))
        $writer.Write([int]$riffSize)
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("WAVE"))
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("fmt "))
        $writer.Write([int]16)
        $writer.Write([int16]1)
        $writer.Write([int16]1)
        $writer.Write([int]$sampleRate)
        $writer.Write([int]($sampleRate * 2))
        $writer.Write([int16]2)
        $writer.Write([int16]16)
        $writer.Write([System.Text.Encoding]::ASCII.GetBytes("data"))
        $writer.Write([int]$dataBytes)
        for ($i = 0; $i -lt $samples; $i++) {
            $writer.Write([int16]0)
        }
    }
    finally {
        $writer.Close()
    }
}

function New-Scenario {
    param(
        [string]$Name,
        [string]$Family,
        [string]$Input,
        [string]$Output,
        [string[]]$Arguments
    )

    [pscustomobject]@{
        index     = -1
        name      = $Name
        family    = $Family
        input     = $Input
        output    = $Output
        arguments = $Arguments
    }
}

function New-StageResult {
    param(
        [string]$Status,
        [string]$Detail = "",
        [string]$Command = "",
        [string]$Artifact = "",
        [object]$ElapsedMs = 0
    )

    $elapsedValue = 0
    try {
        $elapsedValue = [int64](@($ElapsedMs)[0])
    }
    catch {
        $elapsedValue = 0
    }

    [pscustomobject]@{
        status    = $Status
        detail    = $Detail
        command   = $Command
        artifact  = $Artifact
        elapsedMs = $elapsedValue
    }
}

function New-SkippedScenarioResult {
    param(
        [object]$Scenario,
        [string]$Reason
    )

    [pscustomobject]@{
        index           = $Scenario.index
        name            = $Scenario.name
        family          = $Scenario.family
        input           = $Scenario.input
        output          = $Scenario.output
        proofLevel      = "failed"
        mutation        = (New-StageResult -Status "skipped" -Detail $Reason)
        readback        = (New-StageResult -Status "skipped" -Detail $Reason)
        validation      = (New-StageResult -Status "skipped" -Detail $Reason)
        conformance     = (New-StageResult -Status "skipped" -Detail $Reason)
        openXmlSdk      = (New-StageResult -Status "skipped" -Detail $Reason)
        libreOffice     = (New-StageResult -Status "not-run" -Detail "LibreOffice evidence is intentionally separate from Microsoft Office COM proof.")
        microsoftOffice = (New-StageResult -Status "skipped" -Detail $Reason)
    }
}

function Invoke-ScenarioWorker {
    param(
        [object]$Scenario,
        [string]$BinaryPath,
        [string]$DotNetExe,
        [string]$OpenXmlValidatorDll,
        [bool]$RunOpenXmlSdk,
        [bool]$RunConformance,
        [bool]$RequireOpenXmlSdk
    )

    function Quote-WorkerArgument {
        param([string]$Value)

        if ($Value -eq "") {
            return '""'
        }
        if ($Value -match '[\s"]') {
            return '"' + ($Value -replace '"', '\"') + '"'
        }
        return $Value
    }

    function Format-WorkerCommandLine {
        param(
            [string]$FilePath,
            [string[]]$Arguments
        )

        return ((@($FilePath) + $Arguments) | ForEach-Object { Quote-WorkerArgument -Value $_ }) -join " "
    }

    function Invoke-WorkerProcess {
        param(
            [string]$FilePath,
            [string[]]$Arguments
        )

        $timer = [System.Diagnostics.Stopwatch]::StartNew()
        $output = @(& $FilePath @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
        $timer.Stop()

        [pscustomobject]@{
            exitCode  = $exitCode
            elapsedMs = $timer.ElapsedMilliseconds
            output    = (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine)
            command   = (Format-WorkerCommandLine -FilePath $FilePath -Arguments $Arguments)
        }
    }

    function New-WorkerStageResult {
        param(
            [string]$Status,
            [string]$Detail = "",
            [string]$Command = "",
            [string]$Artifact = "",
            [object]$ElapsedMs = 0
        )

        $elapsedValue = 0
        try {
            $elapsedValue = [int64](@($ElapsedMs)[0])
        }
        catch {
            $elapsedValue = 0
        }

        [pscustomObject]@{
            status    = $Status
            detail    = $Detail
            command   = $Command
            artifact  = $Artifact
            elapsedMs = $elapsedValue
        }
    }

    $conformanceStage = if ($RunConformance) {
        New-WorkerStageResult -Status "pending" -Detail "Waiting for repair conformance checks."
    }
    else {
        New-WorkerStageResult -Status "not-run" -Detail "Run with -RunConformance to include repair invariant checks."
    }

    $result = [pscustomobject]@{
        index           = $Scenario.index
        name            = $Scenario.name
        family          = $Scenario.family
        input           = $Scenario.input
        output          = $Scenario.output
        proofLevel      = "strict-validation"
        mutation        = (New-WorkerStageResult -Status "pending")
        readback        = (New-WorkerStageResult -Status "pending")
        validation      = (New-WorkerStageResult -Status "pending")
        conformance     = $conformanceStage
        openXmlSdk      = (New-WorkerStageResult -Status "not-run" -Detail "Open XML SDK validation was not available for this run.")
        libreOffice     = (New-WorkerStageResult -Status "not-run" -Detail "LibreOffice evidence is intentionally separate from Microsoft Office COM proof.")
        microsoftOffice = (New-WorkerStageResult -Status "pending" -Detail "Waiting for desktop Office COM oracle.")
    }

    $mutation = Invoke-WorkerProcess -FilePath $BinaryPath -Arguments $Scenario.arguments
    if ($mutation.exitCode -ne 0) {
        $result.proofLevel = "failed"
        $detail = "Mutation command failed with exit code {0}." -f $mutation.exitCode
        if ($mutation.output -ne "") {
            $detail = "{0} {1}" -f $detail, $mutation.output
        }
        $result.mutation = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $mutation.command -ElapsedMs $mutation.elapsedMs)
        $result.readback = (New-WorkerStageResult -Status "skipped" -Detail "Mutation failed.")
        $result.validation = (New-WorkerStageResult -Status "skipped" -Detail "Mutation failed.")
        $result.conformance = (New-WorkerStageResult -Status "skipped" -Detail "Mutation failed.")
        $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Mutation failed.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Mutation failed.")
        return $result
    }
    $result.mutation = (New-WorkerStageResult -Status "passed" -Detail "Mutation command completed." -Command $mutation.command -ElapsedMs $mutation.elapsedMs)

    $readbackArgs = @("--json", "inspect", $Scenario.output)
    $readback = Invoke-WorkerProcess -FilePath $BinaryPath -Arguments $readbackArgs
    if ($readback.exitCode -ne 0) {
        $result.proofLevel = "failed"
        $detail = "ooxml inspect readback failed with exit code {0}." -f $readback.exitCode
        if ($readback.output -ne "") {
            $detail = "{0} {1}" -f $detail, $readback.output
        }
        $result.readback = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $readback.command -ElapsedMs $readback.elapsedMs)
        $result.validation = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.conformance = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        return $result
    }
    try {
        $readbackJson = $readback.output | ConvertFrom-Json
        $readbackType = [string]$readbackJson.type
    }
    catch {
        $result.proofLevel = "failed"
        $detail = "ooxml inspect readback returned invalid JSON."
        if ($readback.output -ne "") {
            $detail = "{0} {1}" -f $detail, $readback.output
        }
        $result.readback = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $readback.command -ElapsedMs $readback.elapsedMs)
        $result.validation = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.conformance = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        return $result
    }
    if ($readbackType -ne [string]$Scenario.family) {
        $result.proofLevel = "failed"
        $detail = "ooxml inspect readback reported family {0}; expected {1}." -f $readbackType, $Scenario.family
        $result.readback = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $readback.command -ElapsedMs $readback.elapsedMs)
        $result.validation = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.conformance = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Readback failed.")
        return $result
    }
    $result.proofLevel = "saved-readback"
    $result.readback = (New-WorkerStageResult -Status "passed" -Detail ("ooxml inspect read the saved {0} output." -f $readbackType) -Command $readback.command -ElapsedMs $readback.elapsedMs)

    $validationArgs = @("--json", "validate", $Scenario.output, "--strict")
    $validation = Invoke-WorkerProcess -FilePath $BinaryPath -Arguments $validationArgs
    if ($validation.exitCode -ne 0) {
        $result.proofLevel = "failed"
        $detail = "ooxml validate --strict failed with exit code {0}." -f $validation.exitCode
        if ($validation.output -ne "") {
            $detail = "{0} {1}" -f $detail, $validation.output
        }
        $result.validation = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $validation.command -ElapsedMs $validation.elapsedMs)
        $result.conformance = (New-WorkerStageResult -Status "skipped" -Detail "Strict validation failed.")
        $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Strict validation failed.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Strict validation failed.")
        return $result
    }
    $result.validation = (New-WorkerStageResult -Status "passed" -Detail "ooxml validate --strict accepted the edited output." -Command $validation.command -ElapsedMs $validation.elapsedMs)

    if ($RunConformance) {
        $conformanceArgs = @("--json", "conformance", "check", $Scenario.output)
        $conformance = Invoke-WorkerProcess -FilePath $BinaryPath -Arguments $conformanceArgs
        if ($conformance.exitCode -ne 0) {
            $result.proofLevel = "failed"
            $detail = "ooxml conformance check failed with exit code {0}." -f $conformance.exitCode
            if ($conformance.output -ne "") {
                $detail = "{0} {1}" -f $detail, $conformance.output
            }
            $result.conformance = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $conformance.command -ElapsedMs $conformance.elapsedMs)
            $result.openXmlSdk = (New-WorkerStageResult -Status "skipped" -Detail "Conformance check failed.")
            $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Conformance check failed.")
            return $result
        }
        $result.proofLevel = "repair-conformance"
        $result.conformance = (New-WorkerStageResult -Status "passed" -Detail "ooxml conformance check accepted the edited output." -Command $conformance.command -ElapsedMs $conformance.elapsedMs)
    }

    if ($RunOpenXmlSdk) {
        $openXmlArgs = @($OpenXmlValidatorDll, "--json", $Scenario.output)
        $openXml = Invoke-WorkerProcess -FilePath $DotNetExe -Arguments $openXmlArgs
        if ($openXml.exitCode -ne 0) {
            $result.proofLevel = "failed"
            $detail = "Open XML SDK validator failed with exit code {0}." -f $openXml.exitCode
            if ($openXml.output -ne "") {
                $detail = "{0} {1}" -f $detail, $openXml.output
            }
            $result.openXmlSdk = (New-WorkerStageResult -Status "failed" -Detail $detail -Command $openXml.command -Artifact $OpenXmlValidatorDll -ElapsedMs $openXml.elapsedMs)
            return $result
        }
        $result.proofLevel = "openxml-sdk-schema"
        $result.openXmlSdk = (New-WorkerStageResult -Status "passed" -Detail "Microsoft Open XML SDK validator reported 0 schema errors." -Command $openXml.command -Artifact $OpenXmlValidatorDll -ElapsedMs $openXml.elapsedMs)
    }
    elseif ($RequireOpenXmlSdk) {
        $result.proofLevel = "failed"
        $result.openXmlSdk = (New-WorkerStageResult -Status "failed" -Detail "Open XML SDK validation was required but not available.")
        $result.microsoftOffice = (New-WorkerStageResult -Status "skipped" -Detail "Open XML SDK validation was required but not available.")
    }

    return $result
}

function Invoke-ScenarioSet {
    param(
        [object[]]$Scenarios,
        [string]$BinaryPath,
        [string]$DotNetExe,
        [string]$OpenXmlValidatorDll,
        [bool]$RunOpenXmlSdk,
        [bool]$RunConformance,
        [bool]$RequireOpenXmlSdk,
        [int]$Parallelism
    )

    if ($Parallelism -lt 1) {
        $Parallelism = 1
    }

    $queue = New-Object System.Collections.Queue
    foreach ($scenario in $Scenarios) {
        $queue.Enqueue($scenario)
    }

    $results = New-Object System.Collections.Generic.List[object]
    $jobs = @()
    $useThreadJobs = $null -ne (Get-Command Start-ThreadJob -ErrorAction SilentlyContinue)

    while ($queue.Count -gt 0 -or $jobs.Count -gt 0) {
        while ($queue.Count -gt 0 -and $jobs.Count -lt $Parallelism) {
            $scenario = $queue.Dequeue()
            Write-Host ("[{0}] queued mutation/validation" -f $scenario.name)
            if ($Parallelism -eq 1) {
                $results.Add((Invoke-ScenarioWorker -Scenario $scenario -BinaryPath $BinaryPath -DotNetExe $DotNetExe -OpenXmlValidatorDll $OpenXmlValidatorDll -RunOpenXmlSdk $RunOpenXmlSdk -RunConformance $RunConformance -RequireOpenXmlSdk $RequireOpenXmlSdk))
                continue
            }

            if ($useThreadJobs) {
                $job = Start-ThreadJob -ScriptBlock ${function:Invoke-ScenarioWorker} -ArgumentList $scenario, $BinaryPath, $DotNetExe, $OpenXmlValidatorDll, $RunOpenXmlSdk, $RunConformance, $RequireOpenXmlSdk
            }
            else {
                $job = Start-Job -ScriptBlock ${function:Invoke-ScenarioWorker} -ArgumentList $scenario, $BinaryPath, $DotNetExe, $OpenXmlValidatorDll, $RunOpenXmlSdk, $RunConformance, $RequireOpenXmlSdk
            }
            $jobs += $job
        }

        if ($jobs.Count -eq 0) {
            continue
        }

        $done = @(Wait-Job -Job $jobs -Any)
        foreach ($job in $done) {
            $received = @(Receive-Job -Job $job)
            Remove-Job -Job $job -Force
            $jobs = @($jobs | Where-Object { $_.Id -ne $job.Id })

            if ($received.Count -eq 0) {
                $results.Add([pscustomobject]@{
                    index           = 999999
                    name            = "unknown"
                    family          = "unknown"
                    input           = ""
                    output          = ""
                    proofLevel      = "failed"
                    mutation        = (New-StageResult -Status "failed" -Detail "Scenario worker returned no result.")
                    readback        = (New-StageResult -Status "skipped" -Detail "Scenario worker returned no result.")
                    validation      = (New-StageResult -Status "skipped" -Detail "Scenario worker returned no result.")
                    conformance     = (New-StageResult -Status "not-run" -Detail "Broader conformance checks are run separately.")
                    openXmlSdk      = (New-StageResult -Status "skipped" -Detail "Scenario worker returned no result.")
                    libreOffice     = (New-StageResult -Status "not-run" -Detail "LibreOffice evidence is intentionally separate from Microsoft Office COM proof.")
                    microsoftOffice = (New-StageResult -Status "skipped" -Detail "Scenario worker returned no result.")
                })
                continue
            }

            $results.Add($received[-1])
        }
    }

    return @($results | Sort-Object index)
}

function New-PptxSlideExtLstFixture {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourcePath,

        [Parameter(Mandatory = $true)]
        [string]$OutputPath,

        [string]$SlideEntryName = "ppt/slides/slide1.xml"
    )

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem

    Copy-Item -LiteralPath $SourcePath -Destination $OutputPath -Force

    $zip = [System.IO.Compression.ZipFile]::Open($OutputPath, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        $entry = $zip.GetEntry($SlideEntryName)
        if ($null -eq $entry) {
            throw "PPTX fixture is missing $SlideEntryName"
        }

        $readStream = $entry.Open()
        try {
            $reader = New-Object System.IO.StreamReader($readStream, [System.Text.Encoding]::UTF8, $true)
            try {
                $xml = $reader.ReadToEnd()
            }
            finally {
                $reader.Dispose()
            }
        }
        finally {
            $readStream.Dispose()
        }

        if ($xml -notlike "*<p:extLst*") {
            $ext = '<p:extLst><p:ext uri="{BB962C8B-B14F-4D97-AF65-F5344CB8AC3E}"><p14:creationId xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" val="{11111111-1111-1111-1111-111111111111}"/></p:ext></p:extLst>'
            $updated = $xml.Replace("</p:spTree>", ($ext + "</p:spTree>"))
            if ($updated -eq $xml) {
                throw "PPTX fixture slide does not contain a p:spTree close tag"
            }
            $xml = $updated
        }

        $entry.Delete()
        $newEntry = $zip.CreateEntry($SlideEntryName, [System.IO.Compression.CompressionLevel]::Optimal)
        $writeStream = $newEntry.Open()
        try {
            $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
            $writer = New-Object System.IO.StreamWriter($writeStream, $utf8NoBom)
            try {
                $writer.Write($xml)
            }
            finally {
                $writer.Dispose()
            }
        }
        finally {
            $writeStream.Dispose()
        }
    }
    finally {
        $zip.Dispose()
    }

    return $OutputPath
}

$root = (Resolve-Path -LiteralPath $RepoRoot).Path
if ($OutputDir -eq "") {
    # Keep Office-open proof artifacts under the repo tree. Excel COM can
    # treat byte-identical minimal workbooks differently from %TEMP%.
    $OutputDir = Join-Path $root "target\ooxml-office-edit-smoke"
}
$outRoot = [System.IO.Path]::GetFullPath($OutputDir)
$caseDir = Join-Path $outRoot "outputs"
$binDir = Join-Path $outRoot "bin"
$oracleDir = Join-Path $outRoot "office-oracle"

Set-Location -LiteralPath $root

Remove-Item -LiteralPath $outRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $caseDir, $binDir, $oracleDir | Out-Null

if ($GoCache -eq "") {
    $GoCache = Join-Path $env:TEMP "ooxml-go-build-cache"
}
New-Item -ItemType Directory -Force -Path $GoCache | Out-Null
$env:GOCACHE = $GoCache

if ($env:NUGET_PACKAGES -eq $null -or $env:NUGET_PACKAGES -eq "") {
    $env:NUGET_PACKAGES = Join-Path $env:TEMP "ooxml-nuget-packages"
}
New-Item -ItemType Directory -Force -Path $env:NUGET_PACKAGES | Out-Null

if ($env:DOTNET_CLI_HOME -eq $null -or $env:DOTNET_CLI_HOME -eq "") {
    $env:DOTNET_CLI_HOME = Join-Path $env:TEMP "ooxml-dotnet-home"
}
New-Item -ItemType Directory -Force -Path $env:DOTNET_CLI_HOME | Out-Null
$env:DOTNET_CLI_TELEMETRY_OPTOUT = "1"
$env:DOTNET_NOLOGO = "1"
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = "1"

$rangeValuesFile = Join-Path $outRoot "xlsx-ranges-set.csv"
Set-Content -LiteralPath $rangeValuesFile -Encoding ASCII -Value @(
    "Quarter,Revenue",
    "Q1,125",
    "Q2,148"
)

$pivotValuesFile = Join-Path $outRoot "xlsx-pivot-data.csv"
Set-Content -LiteralPath $pivotValuesFile -Encoding ASCII -Value @(
    "Region,Product,Sales",
    "North,A,42",
    "South,A,58",
    "North,B,30",
    "South,B,33"
)

$authoringValuesFile = Join-Path $outRoot "xlsx-authoring-values.json"
Set-Content -LiteralPath $authoringValuesFile -Encoding ASCII -Value @'
[
  ["Region","Account","Units","Unit Price","Revenue"],
  ["North","Enterprise",12,19.95,{"formula":"C2*D2"}],
  ["South","Midmarket",8,24.50,{"formula":"C3*D3"}],
  ["West","Startup",15,9.99,{"formula":"C4*D4"}],
  ["East","Renewal",10,29.00,{"formula":"C5*D5"}]
]
'@

$pptxChartExtLstValuesFile = Join-Path $outRoot "pptx-chart-extlst-values.json"
Set-Content -LiteralPath $pptxChartExtLstValuesFile -Encoding ASCII -Value '[["Region","S1"],["North",10],["South",20]]'

$explicitBinaryPath = $BinaryPath -ne ""
if ($BinaryPath -eq "") {
    $BinaryPath = Join-Path $binDir "ooxml.exe"
}
else {
    $BinaryPath = [System.IO.Path]::GetFullPath($BinaryPath)
}

if (-not $SkipBuild) {
    if ($explicitBinaryPath) {
        throw "Refusing to build Go into explicit -BinaryPath. Omit -BinaryPath to build the Go CLI, or pass -SkipBuild to test the existing binary at: $BinaryPath"
    }
    $go = Resolve-GoExe -Requested $GoExe
    Invoke-Checked -FilePath $go -Arguments @("build", "-buildvcs=false", "-o", $BinaryPath, ".\cmd\ooxml") -Label "build"
}
elseif (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "BinaryPath does not exist and -SkipBuild was set: $BinaryPath"
}

$dotnet = Resolve-DotNetExe -Requested $DotNetExe
if ($OpenXmlValidatorProject -eq "") {
    $OpenXmlValidatorProject = Join-Path $root "tools\openxml-validator\openxml-validator.csproj"
}

$runOpenXmlSdk = $false
$openXmlValidatorDll = ""
$openXmlSetup = (New-StageResult -Status "not-run" -Detail "Open XML SDK validation was skipped.")
if (-not $SkipOpenXmlSdk) {
    if ($dotnet -eq "") {
        $openXmlSetup = (New-StageResult -Status "not-run" -Detail "dotnet was not found on PATH.")
    }
    elseif (-not (Test-Path -LiteralPath $OpenXmlValidatorProject -PathType Leaf)) {
        $openXmlSetup = (New-StageResult -Status "not-run" -Detail ("Open XML validator project was not found: {0}" -f $OpenXmlValidatorProject))
    }
    else {
        $validatorRoot = Split-Path -Parent $OpenXmlValidatorProject
        $openXmlValidatorDll = Join-Path $validatorRoot "bin\Release\net8.0\openxml-validator.dll"
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
        $dotnetAppData = Join-Path $outRoot "dotnet-appdata"
        $dotnetLocalAppData = Join-Path $outRoot "dotnet-localappdata"
        New-Item -ItemType Directory -Force -Path $dotnetAppData, $dotnetLocalAppData | Out-Null
        try {
            $env:APPDATA = $dotnetAppData
            $env:LOCALAPPDATA = $dotnetLocalAppData
            Invoke-Checked -FilePath $dotnet -Arguments @("restore", $OpenXmlValidatorProject, "--configfile", $nugetConfig, "--nologo") -Label "openxml-sdk:restore"
            Invoke-Checked -FilePath $dotnet -Arguments @("build", $OpenXmlValidatorProject, "-c", "Release", "--no-restore", "--nologo") -Label "openxml-sdk:build"
            if (Test-Path -LiteralPath $openXmlValidatorDll -PathType Leaf) {
                $runOpenXmlSdk = $true
                $openXmlSetup = (New-StageResult -Status "passed" -Detail "Open XML SDK validator built successfully." -Command (Format-CommandLine -FilePath $dotnet -Arguments @("build", $OpenXmlValidatorProject, "-c", "Release", "--no-restore", "--nologo")) -Artifact $openXmlValidatorDll)
            }
            else {
                $openXmlSetup = (New-StageResult -Status "not-run" -Detail ("Open XML SDK validator DLL was not found after build: {0}" -f $openXmlValidatorDll))
            }
        }
        catch {
            if ($RequireOpenXmlSdk) {
                throw
            }
            $openXmlSetup = (New-StageResult -Status "not-run" -Detail ("Open XML SDK validator build failed: {0}" -f $_.Exception.Message) -Command (Format-CommandLine -FilePath $dotnet -Arguments @("restore", $OpenXmlValidatorProject, "--configfile", $nugetConfig, "--nologo")))
            Write-Warning $openXmlSetup.detail
        }
        finally {
            $env:APPDATA = $oldAppData
            $env:LOCALAPPDATA = $oldLocalAppData
        }
    }
}

$xlsxMinimal = Join-Path $root "testdata\xlsx\minimal-workbook\workbook.xlsx"
$pptxMinimal = Join-Path $root "testdata\pptx\minimal-title\presentation.pptx"
$pptxSlideAssemblyTarget = Join-Path $root "testdata\pptx\slide-assembly-multi\presentation.pptx"
$pptxSlideAssemblySource = Join-Path $root "testdata\pptx\slide-assembly-notes-media\presentation.pptx"
$pptxPicturePlaceholder = Join-Path $root "testdata\pptx\picture-placeholder\presentation.pptx"
$pptxTableSlide = Join-Path $root "testdata\pptx\table-slide\presentation.pptx"
$pptxHeaderFooter = Join-Path $root "testdata\pptx\header-footer\presentation.pptx"
$docxMinimal = Join-Path $root "testdata\docx\minimal\document.docx"
$docxApplyStyles = Join-Path $root "testdata\docx\apply-styles\document.docx"
$docxTable = Join-Path $root "testdata\docx\table\document.docx"
$docxWithFields = Join-Path $root "testdata\docx\with-fields\document.docx"
$docxWithImage = Join-Path $root "testdata\docx\with-image\document.docx"
$docxWithComments = Join-Path $root "testdata\docx\with-comments\document.docx"
$imageFixture = Join-Path $root "testdata\pptx\template-branded\test-image.png"
$audioFixture = Join-Path $outRoot "smoke-audio.wav"
New-SmokeWavFile -Path $audioFixture

$pptxExtLst = New-PptxSlideExtLstFixture -SourcePath $pptxMinimal -OutputPath (Join-Path $outRoot "pptx-extlst-input.pptx")

$xlsxPivotData = Join-Path $outRoot "xlsx-pivot-data.xlsx"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "xlsx", "ranges", "set", $xlsxMinimal, "--sheet", "1", "--anchor", "A1", "--data-format", "csv", "--values-file", $pivotValuesFile, "--out", $xlsxPivotData) -Label "stage:xlsx-pivot-data"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "validate", $xlsxPivotData, "--strict") -Label "validate:stage:xlsx-pivot-data"

$xlsxPivotNamedData = Join-Path $outRoot "xlsx-pivot-named-data.xlsx"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "xlsx", "names", "add", $xlsxPivotData, "--name", "PivotSource", "--sheet", "1", "--range", "A1:C5", "--comment", "Pivot smoke source", "--out", $xlsxPivotNamedData) -Label "stage:xlsx-pivot-named-data"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "validate", $xlsxPivotNamedData, "--strict") -Label "validate:stage:xlsx-pivot-named-data"

$xlsxAuthoringSeed = Join-Path $outRoot "xlsx-authoring-seed.xlsx"
$xlsxAuthoringData = Join-Path $outRoot "xlsx-authoring-data.xlsx"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "xlsx", "scaffold", $xlsxAuthoringSeed, "--sheet", "Sales Ops", "--force") -Label "stage:xlsx-authoring-seed"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "xlsx", "ranges", "set", $xlsxAuthoringSeed, "--sheet", "Sales Ops", "--range", "A1:E5", "--values-file", $authoringValuesFile, "--out", $xlsxAuthoringData) -Label "stage:xlsx-authoring-data"
Invoke-Checked -FilePath $BinaryPath -Arguments @("--json", "validate", $xlsxAuthoringData, "--strict") -Label "validate:stage:xlsx-authoring-data"

$docxTableHash = Get-DocxTableHash -BinaryPath $BinaryPath -DocumentPath $docxTable -Table 1
$docxBlock1Hash = Get-DocxBlockHash -BinaryPath $BinaryPath -DocumentPath $docxMinimal -Block 1
$docxApplyBlock1Hash = Get-DocxBlockHash -BinaryPath $BinaryPath -DocumentPath $docxApplyStyles -Block 1
$docxComment0Hash = Get-DocxCommentHash -BinaryPath $BinaryPath -DocumentPath $docxWithComments -CommentID 0

$scenarios = @(
    (New-Scenario `
        -Name "xlsx-scaffold" `
        -Family "xlsx" `
        -Input "" `
        -Output (Join-Path $caseDir "xlsx-scaffold.xlsx") `
        -Arguments @("--json", "xlsx", "scaffold", (Join-Path $caseDir "xlsx-scaffold.xlsx"), "--sheet", "OfficeScaffold")),

    (New-Scenario `
        -Name "xlsx-cells-set" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-cells-set.xlsx") `
        -Arguments @("--json", "xlsx", "cells", "set", $xlsxMinimal, "--sheet", "1", "--cell", "D1", "--value", "Office edit smoke", "--out", (Join-Path $caseDir "xlsx-cells-set.xlsx"))),

    (New-Scenario `
        -Name "xlsx-ranges-set" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-ranges-set.xlsx") `
        -Arguments @("--json", "xlsx", "ranges", "set", $xlsxMinimal, "--sheet", "1", "--anchor", "A3", "--data-format", "csv", "--values-file", $rangeValuesFile, "--out", (Join-Path $caseDir "xlsx-ranges-set.xlsx"))),

    (New-Scenario `
        -Name "xlsx-ranges-set-formulas-from-scaffold" `
        -Family "xlsx" `
        -Input $xlsxAuthoringSeed `
        -Output (Join-Path $caseDir "xlsx-ranges-set-formulas-from-scaffold.xlsx") `
        -Arguments @("--json", "xlsx", "ranges", "set", $xlsxAuthoringSeed, "--sheet", "Sales Ops", "--range", "A1:E5", "--values-file", $authoringValuesFile, "--out", (Join-Path $caseDir "xlsx-ranges-set-formulas-from-scaffold.xlsx"))),

    (New-Scenario `
        -Name "xlsx-tables-create-from-scaffold" `
        -Family "xlsx" `
        -Input $xlsxAuthoringData `
        -Output (Join-Path $caseDir "xlsx-tables-create-from-scaffold.xlsx") `
        -Arguments @("--json", "xlsx", "tables", "create", $xlsxAuthoringData, "--sheet", "Sales Ops", "--range", "A1:E5", "--table", "SalesOps", "--style", "TableStyleMedium4", "--out", (Join-Path $caseDir "xlsx-tables-create-from-scaffold.xlsx"))),

    (New-Scenario `
        -Name "xlsx-conditional-formats-add-from-scaffold" `
        -Family "xlsx" `
        -Input $xlsxAuthoringData `
        -Output (Join-Path $caseDir "xlsx-conditional-formats-add-from-scaffold.xlsx") `
        -Arguments @("--json", "xlsx", "conditional-formats", "add", $xlsxAuthoringData, "--sheet", "Sales Ops", "--range", "E2:E5", "--type", "color-scale", "--cfvo", "min", "--cfvo", "percentile:50", "--cfvo", "max", "--color", "F8696B", "--color", "FFEB84", "--color", "63BE7B", "--priority", "1", "--out", (Join-Path $caseDir "xlsx-conditional-formats-add-from-scaffold.xlsx"))),

    (New-Scenario `
        -Name "xlsx-comments-add" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-comments-add.xlsx") `
        -Arguments @("--json", "xlsx", "comments", "add", $xlsxMinimal, "--sheet", "1", "--cell", "B2", "--author", "OOXML Smoke", "--text", "Excel comment smoke", "--out", (Join-Path $caseDir "xlsx-comments-add.xlsx"))),

    (New-Scenario `
        -Name "xlsx-data-validation-create-list" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-data-validation-create-list.xlsx") `
        -Arguments @("--json", "xlsx", "data-validations", "create", $xlsxMinimal, "--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "Red,Green,Blue", "--show-input-message", "--input-title", "Pick", "--input-message", "Choose a color", "--out", (Join-Path $caseDir "xlsx-data-validation-create-list.xlsx"))),

    (New-Scenario `
        -Name "xlsx-charts-create" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-charts-create.xlsx") `
        -Arguments @("--json", "xlsx", "charts", "create", $xlsxPivotData, "--type", "line", "--sheet", "1", "--range", "A1:C5", "--expect-source-range", "A1:C5", "--title", "Office Chart Smoke", "--anchor", "E18", "--out", (Join-Path $caseDir "xlsx-charts-create.xlsx"))),

    (New-Scenario `
        -Name "xlsx-ranges-set-format" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-ranges-set-format.xlsx") `
        -Arguments @("--json", "xlsx", "ranges", "set-format", $xlsxPivotData, "--sheet", "1", "--range", "C2:C5", "--preset", "currency", "--decimals", "0", "--out", (Join-Path $caseDir "xlsx-ranges-set-format.xlsx"))),

    (New-Scenario `
        -Name "xlsx-sheets-add" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-sheets-add.xlsx") `
        -Arguments @("--json", "xlsx", "sheets", "add", $xlsxMinimal, "--name", "OfficeSmokeSheet", "--out", (Join-Path $caseDir "xlsx-sheets-add.xlsx"))),

    (New-Scenario `
        -Name "xlsx-rows-insert" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-rows-insert.xlsx") `
        -Arguments @("--json", "xlsx", "rows", "insert", $xlsxMinimal, "--sheet", "1", "--at", "2", "--count", "2", "--out", (Join-Path $caseDir "xlsx-rows-insert.xlsx"))),

    (New-Scenario `
        -Name "xlsx-rows-delete" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-rows-delete.xlsx") `
        -Arguments @("--json", "xlsx", "rows", "delete", $xlsxPivotData, "--sheet", "1", "--row", "4", "--count", "1", "--out", (Join-Path $caseDir "xlsx-rows-delete.xlsx"))),

    (New-Scenario `
        -Name "xlsx-cols-insert" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-cols-insert.xlsx") `
        -Arguments @("--json", "xlsx", "cols", "insert", $xlsxMinimal, "--sheet", "1", "--at", "B", "--count", "2", "--out", (Join-Path $caseDir "xlsx-cols-insert.xlsx"))),

    (New-Scenario `
        -Name "xlsx-cols-delete" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-cols-delete.xlsx") `
        -Arguments @("--json", "xlsx", "cols", "delete", $xlsxPivotData, "--sheet", "1", "--col", "C", "--count", "1", "--out", (Join-Path $caseDir "xlsx-cols-delete.xlsx"))),

    (New-Scenario `
        -Name "xlsx-freeze-set" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-freeze-set.xlsx") `
        -Arguments @("--json", "xlsx", "freeze", "set", $xlsxMinimal, "--sheet", "1", "--rows", "1", "--cols", "1", "--expect-state", "none", "--out", (Join-Path $caseDir "xlsx-freeze-set.xlsx"))),

    (New-Scenario `
        -Name "xlsx-filters-sorts-set-autofilter" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-filters-sorts-set-autofilter.xlsx") `
        -Arguments @("--json", "xlsx", "filters-sorts", "set-autofilter", $xlsxPivotData, "--sheet", "1", "--range", "A1:C5", "--out", (Join-Path $caseDir "xlsx-filters-sorts-set-autofilter.xlsx"))),

    (New-Scenario `
        -Name "xlsx-filters-sorts-set-sort" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-filters-sorts-set-sort.xlsx") `
        -Arguments @("--json", "xlsx", "filters-sorts", "set-sort", $xlsxPivotData, "--sheet", "1", "--ref", "A1:C5", "--column", "C", "--descending", "--out", (Join-Path $caseDir "xlsx-filters-sorts-set-sort.xlsx"))),

    (New-Scenario `
        -Name "xlsx-names-add" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-names-add.xlsx") `
        -Arguments @("--json", "xlsx", "names", "add", $xlsxPivotData, "--name", "OfficeSmokeRange", "--sheet", "1", "--range", "A1:C5", "--comment", "Office smoke defined name", "--out", (Join-Path $caseDir "xlsx-names-add.xlsx"))),

    (New-Scenario `
        -Name "pptx-scaffold" `
        -Family "pptx" `
        -Input "" `
        -Output (Join-Path $caseDir "pptx-scaffold.pptx") `
        -Arguments @("--json", "pptx", "scaffold", (Join-Path $caseDir "pptx-scaffold.pptx"), "--title", "Office Scaffold", "--subtitle", "Opened by PowerPoint")),

    (New-Scenario `
        -Name "pptx-replace-text" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-replace-text.pptx") `
        -Arguments @("--json", "pptx", "replace", "text-occurrences", $pptxMinimal, "--match-text", "Minimal Title Slide", "--new-text", "Office Edit Smoke", "--expect-count", "1", "--out", (Join-Path $caseDir "pptx-replace-text.pptx"))),

    (New-Scenario `
        -Name "pptx-add-textbox" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-add-textbox.pptx") `
        -Arguments @("--json", "pptx", "add-textbox", $pptxMinimal, "--slide", "1", "--text", "Opened by PowerPoint", "--x", "914400", "--y", "4572000", "--cx", "5486400", "--cy", "685800", "--font-size", "18", "--color", "1F4E79", "--out", (Join-Path $caseDir "pptx-add-textbox.pptx"))),

    (New-Scenario `
        -Name "pptx-add-textbox-extlst" `
        -Family "pptx" `
        -Input $pptxExtLst `
        -Output (Join-Path $caseDir "pptx-add-textbox-extlst.pptx") `
        -Arguments @("--json", "pptx", "add-textbox", $pptxExtLst, "--slide", "1", "--text", "Inserted before extLst", "--x", "914400", "--y", "5486400", "--cx", "5486400", "--cy", "457200", "--font-size", "16", "--color", "2E7D32", "--out", (Join-Path $caseDir "pptx-add-textbox-extlst.pptx"))),

    (New-Scenario `
        -Name "pptx-chart-create-extlst" `
        -Family "pptx" `
        -Input $pptxExtLst `
        -Output (Join-Path $caseDir "pptx-chart-create-extlst.pptx") `
        -Arguments @("--json", "pptx", "charts", "create", $pptxExtLst, "--slide", "1", "--type", "bar", "--title", "ExtLst Chart", "--values-file", $pptxChartExtLstValuesFile, "--x", "914400", "--y", "914400", "--cx", "5486400", "--cy", "2743200", "--out", (Join-Path $caseDir "pptx-chart-create-extlst.pptx"))),

    (New-Scenario `
        -Name "pptx-comments-add" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-comments-add.pptx") `
        -Arguments @("--json", "pptx", "comments", "add", $pptxMinimal, "--slide", "1", "--author", "OOXML Smoke", "--initials", "OS", "--date", "2026-01-01T00:00:00Z", "--text", "PowerPoint comment smoke", "--out", (Join-Path $caseDir "pptx-comments-add.pptx"))),

    (New-Scenario `
        -Name "pptx-notes-set-create" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-notes-set-create.pptx") `
        -Arguments @("--json", "pptx", "notes", "set", $pptxMinimal, "--slide", "1", "--text", "Office smoke notes`nSecond line", "--out", (Join-Path $caseDir "pptx-notes-set-create.pptx"))),

    (New-Scenario `
        -Name "pptx-fields-set" `
        -Family "pptx" `
        -Input $pptxHeaderFooter `
        -Output (Join-Path $caseDir "pptx-fields-set.pptx") `
        -Arguments @("--json", "pptx", "fields", "set", $pptxHeaderFooter, "--footer", "Confidential", "--show-slide-number=false", "--date-format", "date-only", "--out", (Join-Path $caseDir "pptx-fields-set.pptx"))),

    (New-Scenario `
        -Name "pptx-animations-add-appear" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-animations-add-appear.pptx") `
        -Arguments @("--json", "pptx", "animations", "add", $pptxMinimal, "--slide", "1", "--shape", "shape:2", "--effect", "appear", "--out", (Join-Path $caseDir "pptx-animations-add-appear.pptx"))),

    (New-Scenario `
        -Name "pptx-shapes-set-bounds" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-shapes-set-bounds.pptx") `
        -Arguments @("--json", "pptx", "shapes", "set-bounds", $pptxMinimal, "--slide", "1", "--target", "title", "--bounds", "914400,914400,5486400,685800", "--out", (Join-Path $caseDir "pptx-shapes-set-bounds.pptx"))),

    (New-Scenario `
        -Name "pptx-place-image" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-place-image.pptx") `
        -Arguments @("--json", "pptx", "place", "image", $pptxMinimal, "--slide", "1", "--image", $imageFixture, "--x", "914400", "--y", "4114800", "--cx", "1828800", "--cy", "1371600", "--name", "Office Smoke Image", "--fit-mode", "contain", "--out", (Join-Path $caseDir "pptx-place-image.pptx"))),

    (New-Scenario `
        -Name "pptx-media-add-audio" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-media-add-audio.pptx") `
        -Arguments @("--json", "pptx", "media", "add", $pptxMinimal, "--slide", "1", "--file", $audioFixture, "--kind", "audio", "--name", "Office Smoke Audio", "--x", "914400", "--y", "5029200", "--cx", "914400", "--cy", "914400", "--play-trigger", "none", "--out", (Join-Path $caseDir "pptx-media-add-audio.pptx"))),

    (New-Scenario `
        -Name "pptx-tables-set-cell" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-tables-set-cell.pptx") `
        -Arguments @("--json", "pptx", "tables", "set-cell", $pptxTableSlide, "--slide", "2", "--target", "table:1", "--row", "2", "--col", "2", "--text", "Office table smoke", "--out", (Join-Path $caseDir "pptx-tables-set-cell.pptx"))),

    (New-Scenario `
        -Name "pptx-tables-insert-row" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-tables-insert-row.pptx") `
        -Arguments @("--json", "pptx", "tables", "insert-row", $pptxTableSlide, "--slide", "2", "--target", "table:1", "--at", "1", "--out", (Join-Path $caseDir "pptx-tables-insert-row.pptx"))),

    (New-Scenario `
        -Name "pptx-tables-delete-row" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-tables-delete-row.pptx") `
        -Arguments @("--json", "pptx", "tables", "delete-row", $pptxTableSlide, "--slide", "2", "--target", "table:1", "--row", "2", "--out", (Join-Path $caseDir "pptx-tables-delete-row.pptx"))),

    (New-Scenario `
        -Name "pptx-tables-insert-col" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-tables-insert-col.pptx") `
        -Arguments @("--json", "pptx", "tables", "insert-col", $pptxTableSlide, "--slide", "2", "--target", "table:1", "--at", "1", "--width-emu", "1234567", "--out", (Join-Path $caseDir "pptx-tables-insert-col.pptx"))),

    (New-Scenario `
        -Name "pptx-tables-delete-col" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-tables-delete-col.pptx") `
        -Arguments @("--json", "pptx", "tables", "delete-col", $pptxTableSlide, "--slide", "2", "--target", "table:1", "--col", "2", "--out", (Join-Path $caseDir "pptx-tables-delete-col.pptx"))),

    (New-Scenario `
        -Name "pptx-clone-slide" `
        -Family "pptx" `
        -Input $pptxMinimal `
        -Output (Join-Path $caseDir "pptx-clone-slide.pptx") `
        -Arguments @("--json", "pptx", "clone-slide", $pptxMinimal, "--slide", "1", "--out", (Join-Path $caseDir "pptx-clone-slide.pptx"))),

    (New-Scenario `
        -Name "docx-replace" `
        -Family "docx" `
        -Input $docxMinimal `
        -Output (Join-Path $caseDir "docx-replace.docx") `
        -Arguments @("--json", "docx", "replace", $docxMinimal, "--find", "Hello world", "--replace", "Hello from ooxml office smoke", "--expect-count", "1", "--out", (Join-Path $caseDir "docx-replace.docx"))),

    (New-Scenario `
        -Name "docx-scaffold" `
        -Family "docx" `
        -Input "" `
        -Output (Join-Path $caseDir "docx-scaffold.docx") `
        -Arguments @("--json", "docx", "scaffold", (Join-Path $caseDir "docx-scaffold.docx"), "--text", "Office scaffold")),

    (New-Scenario `
        -Name "docx-comments-add" `
        -Family "docx" `
        -Input $docxMinimal `
        -Output (Join-Path $caseDir "docx-comments-add.docx") `
        -Arguments @("--json", "docx", "comments", "add", $docxMinimal, "--anchor-block", "1", "--author", "OOXML Smoke", "--initials", "OS", "--date", "2026-01-01T00:00:00Z", "--text", "Word comment smoke", "--out", (Join-Path $caseDir "docx-comments-add.docx"))),

    (New-Scenario `
        -Name "docx-fields-set-result" `
        -Family "docx" `
        -Input $docxWithFields `
        -Output (Join-Path $caseDir "docx-fields-set-result.docx") `
        -Arguments @("--json", "docx", "fields", "set-result", $docxWithFields, "--selector", "header1:1:0", "--result", "11", "--out", (Join-Path $caseDir "docx-fields-set-result.docx"))),

    (New-Scenario `
        -Name "docx-header-set-text" `
        -Family "docx" `
        -Input $docxMinimal `
        -Output (Join-Path $caseDir "docx-header-set-text.docx") `
        -Arguments @("--json", "docx", "headers", "set-text", $docxMinimal, "--type", "default", "--section", "1", "--index", "1", "--text", "Office header smoke", "--out", (Join-Path $caseDir "docx-header-set-text.docx"))),

    (New-Scenario `
        -Name "docx-tables-set-cell" `
        -Family "docx" `
        -Input $docxTable `
        -Output (Join-Path $caseDir "docx-tables-set-cell.docx") `
        -Arguments @("--json", "docx", "tables", "set-cell", $docxTable, "--table", "1", "--row", "1", "--col", "2", "--expect-hash", $docxTableHash, "--text", "Office table smoke", "--out", (Join-Path $caseDir "docx-tables-set-cell.docx"))),

    (New-Scenario `
        -Name "docx-tables-clear-cell" `
        -Family "docx" `
        -Input $docxTable `
        -Output (Join-Path $caseDir "docx-tables-clear-cell.docx") `
        -Arguments @("--json", "docx", "tables", "clear-cell", $docxTable, "--table", "1", "--row", "2", "--col", "1", "--expect-hash", $docxTableHash, "--out", (Join-Path $caseDir "docx-tables-clear-cell.docx"))),

    (New-Scenario `
        -Name "docx-tables-insert-row" `
        -Family "docx" `
        -Input $docxTable `
        -Output (Join-Path $caseDir "docx-tables-insert-row.docx") `
        -Arguments @("--json", "docx", "tables", "insert-row", $docxTable, "--table", "1", "--at", "2", "--expect-hash", $docxTableHash, "--out", (Join-Path $caseDir "docx-tables-insert-row.docx"))),

    (New-Scenario `
        -Name "docx-tables-delete-row" `
        -Family "docx" `
        -Input $docxTable `
        -Output (Join-Path $caseDir "docx-tables-delete-row.docx") `
        -Arguments @("--json", "docx", "tables", "delete-row", $docxTable, "--table", "1", "--row", "2", "--expect-hash", $docxTableHash, "--out", (Join-Path $caseDir "docx-tables-delete-row.docx"))),

    (New-Scenario `
        -Name "docx-blocks-insert-after" `
        -Family "docx" `
        -Input $docxMinimal `
        -Output (Join-Path $caseDir "docx-blocks-insert-after.docx") `
        -Arguments @("--json", "docx", "blocks", "insert-after", $docxMinimal, "--block", "1", "--expect-hash", $docxBlock1Hash, "--text", "Office body block smoke", "--out", (Join-Path $caseDir "docx-blocks-insert-after.docx"))),

    (New-Scenario `
        -Name "docx-styles-apply-paragraph" `
        -Family "docx" `
        -Input $docxApplyStyles `
        -Output (Join-Path $caseDir "docx-styles-apply-paragraph.docx") `
        -Arguments @("--json", "docx", "styles", "apply", $docxApplyStyles, "--index", "1", "--target", "paragraph", "--style", "Heading2", "--expect-hash", $docxApplyBlock1Hash, "--out", (Join-Path $caseDir "docx-styles-apply-paragraph.docx"))),

    (New-Scenario `
        -Name "docx-images-insert" `
        -Family "docx" `
        -Input $docxMinimal `
        -Output (Join-Path $caseDir "docx-images-insert.docx") `
        -Arguments @("--json", "docx", "images", "insert", $docxMinimal, "--after", "1", "--expect-hash", $docxBlock1Hash, "--file", $imageFixture, "--width", "1828800", "--height", "1371600", "--out", (Join-Path $caseDir "docx-images-insert.docx"))),

    (New-Scenario `
        -Name "docx-comments-edit" `
        -Family "docx" `
        -Input $docxWithComments `
        -Output (Join-Path $caseDir "docx-comments-edit.docx") `
        -Arguments @("--json", "docx", "comments", "edit", $docxWithComments, "--comment-id", "0", "--expect-hash", $docxComment0Hash, "--text", "Edited Word comment smoke", "--author", "OOXML Smoke", "--date", "2026-01-01T00:00:00Z", "--out", (Join-Path $caseDir "docx-comments-edit.docx"))),

    (New-Scenario `
        -Name "docx-comments-remove" `
        -Family "docx" `
        -Input $docxWithComments `
        -Output (Join-Path $caseDir "docx-comments-remove.docx") `
        -Arguments @("--json", "docx", "comments", "remove", $docxWithComments, "--comment-id", "0", "--expect-hash", $docxComment0Hash, "--out", (Join-Path $caseDir "docx-comments-remove.docx"))),

    (New-Scenario `
        -Name "pptx-import-slide-notes-media" `
        -Family "pptx" `
        -Input $pptxSlideAssemblyTarget `
        -Output (Join-Path $caseDir "pptx-import-slide-notes-media.pptx") `
        -Arguments @("--json", "pptx", "slides", "import-slide", $pptxSlideAssemblyTarget, "--source", $pptxSlideAssemblySource, "--slide", "2", "--out", (Join-Path $caseDir "pptx-import-slide-notes-media.pptx"))),

    (New-Scenario `
        -Name "pptx-merge-notes-media" `
        -Family "pptx" `
        -Input $pptxSlideAssemblyTarget `
        -Output (Join-Path $caseDir "pptx-merge-notes-media.pptx") `
        -Arguments @("--json", "pptx", "slides", "merge", $pptxSlideAssemblyTarget, $pptxSlideAssemblySource, "--layout-policy", "import", "--theme-policy", "import", "--out", (Join-Path $caseDir "pptx-merge-notes-media.pptx"))),

    (New-Scenario `
        -Name "pptx-new-slide-image-slot" `
        -Family "pptx" `
        -Input $pptxPicturePlaceholder `
        -Output (Join-Path $caseDir "pptx-new-slide-image-slot.pptx") `
        -Arguments @("--json", "pptx", "new-slide-from-layout", $pptxPicturePlaceholder, "--layout", "9", "--set-image-slot", ("pic:1=" + $imageFixture), "--image-fit", "cover", "--out", (Join-Path $caseDir "pptx-new-slide-image-slot.pptx"))),

    (New-Scenario `
        -Name "pptx-table-update-from-xlsx" `
        -Family "pptx" `
        -Input $pptxTableSlide `
        -Output (Join-Path $caseDir "pptx-table-update-from-xlsx.pptx") `
        -Arguments @("--json", "pptx", "tables", "update-from-xlsx", $pptxTableSlide, "--workbook", $xlsxPivotData, "--sheet", "Sheet1", "--range", "A1:C3", "--formula-mode", "value", "--slide", "2", "--target", "table:1", "--out", (Join-Path $caseDir "pptx-table-update-from-xlsx.pptx"))),

    (New-Scenario `
        -Name "xlsx-hyperlink-add" `
        -Family "xlsx" `
        -Input $xlsxMinimal `
        -Output (Join-Path $caseDir "xlsx-hyperlink-add.xlsx") `
        -Arguments @("--json", "xlsx", "hyperlinks", "add", $xlsxMinimal, "--sheet", "1", "--cell", "A1", "--url", "https://example.com", "--tooltip", "Office smoke", "--out", (Join-Path $caseDir "xlsx-hyperlink-add.xlsx"))),

    (New-Scenario `
        -Name "xlsx-pivot-create" `
        -Family "xlsx" `
        -Input $xlsxPivotData `
        -Output (Join-Path $caseDir "xlsx-pivot-create.xlsx") `
        -Arguments @("--json", "xlsx", "pivots", "create", $xlsxPivotData, "--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--values", "Sales:sum", "--anchor", "F1", "--out", (Join-Path $caseDir "xlsx-pivot-create.xlsx"))),

    (New-Scenario `
        -Name "xlsx-pivot-create-after-names" `
        -Family "xlsx" `
        -Input $xlsxPivotNamedData `
        -Output (Join-Path $caseDir "xlsx-pivot-create-after-names.xlsx") `
        -Arguments @("--json", "xlsx", "pivots", "create", $xlsxPivotNamedData, "--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--values", "Sales:sum", "--anchor", "F1", "--out", (Join-Path $caseDir "xlsx-pivot-create-after-names.xlsx"))),

    (New-Scenario `
        -Name "docx-image-replace" `
        -Family "docx" `
        -Input $docxWithImage `
        -Output (Join-Path $caseDir "docx-image-replace.docx") `
        -Arguments @("--json", "docx", "images", "replace", $docxWithImage, "--image", "1", "--file", $imageFixture, "--out", (Join-Path $caseDir "docx-image-replace.docx")))
)

if ($ScenarioName.Count -gt 0) {
    $requestedScenarioNames = @($ScenarioName | Where-Object { $null -ne $_ -and $_ -ne "" })
    $knownScenarioNames = @{}
    foreach ($scenario in $scenarios) {
        $knownScenarioNames[[string]$scenario.name] = $true
    }

    $missingScenarioNames = @($requestedScenarioNames | Where-Object { -not $knownScenarioNames.ContainsKey([string]$_) })
    if ($missingScenarioNames.Count -gt 0) {
        $knownList = (($knownScenarioNames.Keys | Sort-Object) -join ", ")
        $missingList = (($missingScenarioNames | Sort-Object) -join ", ")
        throw "Unknown ScenarioName value(s): $missingList. Known scenarios: $knownList"
    }

    $wantedScenarioNames = @{}
    foreach ($name in $requestedScenarioNames) {
        $wantedScenarioNames[[string]$name] = $true
    }
    $scenarios = @($scenarios | Where-Object { $wantedScenarioNames.ContainsKey([string]$_.name) })
}

for ($i = 0; $i -lt $scenarios.Count; $i++) {
    $scenarios[$i].index = $i
}

Write-Host ("[scenarios] running {0} mutation/validation scenario(s) with parallelism {1}" -f $scenarios.Count, $MutationParallelism)
$results = @(Invoke-ScenarioSet -Scenarios $scenarios -BinaryPath $BinaryPath -DotNetExe $dotnet -OpenXmlValidatorDll $openXmlValidatorDll -RunOpenXmlSdk $runOpenXmlSdk -RunConformance ([bool]$RunConformance) -RequireOpenXmlSdk ([bool]$RequireOpenXmlSdk) -Parallelism $MutationParallelism)

$oracle = Join-Path $root "tools\windows-office-oracle.ps1"
$oracleInputs = @($results | Where-Object {
    $_.mutation.status -eq "passed" -and
    $_.validation.status -eq "passed" -and
    ((-not $RunConformance) -or $_.conformance.status -eq "passed") -and
    $_.openXmlSdk.status -ne "failed" -and
    (Test-Path -LiteralPath $_.output -PathType Leaf)
} | ForEach-Object { $_.output })
$oracleExitCode = 0
$oracleSummaryPath = Join-Path $oracleDir "summary.json"

if ($SkipOffice) {
    Write-Host "[office-oracle] skipped by -SkipOffice"
}
elseif ($oracleInputs.Count -gt 0) {
    Write-Host ("[office-oracle] {0} -RepoRoot {1} -InputFile <{2} file(s)> -OutputDir {3} -TimeoutSeconds {4}" -f $oracle, $root, $oracleInputs.Count, $oracleDir, $OfficeOracleTimeoutSeconds)
    if ($Visible) {
        & $oracle -RepoRoot $root -InputFile $oracleInputs -OutputDir $oracleDir -TimeoutSeconds $OfficeOracleTimeoutSeconds -Visible
    }
    else {
        & $oracle -RepoRoot $root -InputFile $oracleInputs -OutputDir $oracleDir -TimeoutSeconds $OfficeOracleTimeoutSeconds
    }
    $oracleExitCode = $LASTEXITCODE
}
else {
    Write-Warning "No edited outputs reached the Office oracle stage."
}

if ($SkipOffice) {
    foreach ($result in $results) {
        if ($result.microsoftOffice.status -eq "pending") {
            $result.microsoftOffice = (New-StageResult -Status "skipped" -Detail "Skipped by -SkipOffice.")
        }
    }
}
elseif ($oracleInputs.Count -gt 0 -and -not (Test-Path -LiteralPath $oracleSummaryPath -PathType Leaf)) {
    foreach ($result in $results) {
        if ($result.microsoftOffice.status -eq "pending") {
            $result.microsoftOffice = (New-StageResult -Status "missing" -Detail ("office-oracle did not write summary.json; exit code {0}" -f $oracleExitCode))
        }
    }
}
elseif (Test-Path -LiteralPath $oracleSummaryPath -PathType Leaf) {
    $officeRows = @(Get-Content -LiteralPath $oracleSummaryPath -Raw | ConvertFrom-Json)
    if ($officeRows.Count -eq 1 -and $officeRows[0] -is [System.Array]) {
        $officeRows = @($officeRows[0])
    }
    foreach ($result in $results) {
        if ($result.microsoftOffice.status -ne "pending") {
            continue
        }

        $office = @($officeRows | Where-Object { $_.file -eq $result.output })[0]
        if ($null -eq $office) {
            $result.microsoftOffice = (New-StageResult -Status "missing" -Detail "No Office oracle result found." -Artifact $oracleSummaryPath)
            continue
        }
        if ($office.status -eq "passed") {
            $result.proofLevel = "microsoft-office-com-open"
            $detail = "{0} opened the edited output without repair/failure." -f $office.officeApplication
            if ($office.officeVersion -ne "") {
                $detail = "{0} Office version {1}" -f $detail, $office.officeVersion
                if ($office.officeBuild -ne "") {
                    $detail = "{0} build {1}" -f $detail, $office.officeBuild
                }
                $detail = "{0}." -f $detail
            }
            $result.microsoftOffice = (New-StageResult -Status "passed" -Detail $detail -Artifact $oracleSummaryPath -ElapsedMs $office.elapsedMs)
        }
        else {
            $result.proofLevel = "failed"
            $result.microsoftOffice = (New-StageResult -Status $office.status -Detail $office.errorMessage -Artifact $oracleSummaryPath -ElapsedMs $office.elapsedMs)
        }
    }
}

foreach ($result in $results) {
    if ($result.microsoftOffice.status -eq "pending") {
        $result.microsoftOffice = (New-StageResult -Status "skipped" -Detail "Earlier validation stage did not reach Office oracle.")
    }
}

$failed = @($results | Where-Object {
    $_.mutation.status -ne "passed" -or
    $_.readback.status -ne "passed" -or
    $_.validation.status -ne "passed" -or
    ($RunConformance -and $_.conformance.status -ne "passed") -or
    $_.openXmlSdk.status -eq "failed" -or
    ($RequireOpenXmlSdk -and $_.openXmlSdk.status -ne "passed") -or
    ((-not $SkipOffice) -and $_.microsoftOffice.status -ne "passed")
})

$overallStatus = "passed"
$overallProofLevel = "microsoft-office-com-open"
if ($SkipOffice -and $RunConformance -and $runOpenXmlSdk) {
    $overallProofLevel = "openxml-sdk-schema"
}
elseif ($SkipOffice -and $RunConformance) {
    $overallProofLevel = "repair-conformance"
}
elseif ($SkipOffice -and $runOpenXmlSdk) {
    $overallProofLevel = "openxml-sdk-schema"
}
elseif ($SkipOffice) {
    $overallProofLevel = "strict-validation"
}
if ($failed.Count -gt 0 -or $oracleExitCode -ne 0) {
    $overallStatus = "failed"
    $overallProofLevel = "failed"
}

$summary = [pscustomobject]@{
    timestampUtc         = [DateTime]::UtcNow.ToString("o")
    status               = $overallStatus
    proofLevel           = $overallProofLevel
    binary               = $BinaryPath
    outputDir            = $outRoot
    mutationParallelism  = $MutationParallelism
    scenarioNameFilter   = @($ScenarioName)
    scenarioCount        = $results.Count
    passedCount          = ($results.Count - $failed.Count)
    failedCount          = $failed.Count
    proofLevels          = @(
        [pscustomobject]@{ id = "saved-readback"; description = "ooxml inspect reopened the edited package and reported the expected Office family." },
        [pscustomobject]@{ id = "strict-validation"; description = "ooxml validate --strict accepted the edited package." },
        [pscustomobject]@{ id = "repair-conformance"; description = "ooxml conformance check accepted the edited package's repair-focused invariants." },
        [pscustomobject]@{ id = "openxml-sdk-schema"; description = "Microsoft Open XML SDK schema validator reported 0 errors." },
        [pscustomobject]@{ id = "microsoft-office-com-open"; description = "Desktop Microsoft Word, Excel, or PowerPoint opened the edited output without repair/failure." }
    )
    openXmlSdkSetup      = $openXmlSetup
    scenarios            = $results
    officeOracleExitCode = $oracleExitCode
    officeOracleTimeoutSeconds = $OfficeOracleTimeoutSeconds
    officeSummary        = $oracleSummaryPath
}

$summaryPath = Join-Path $outRoot "summary.json"
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8

$artifactProofMatrix = [pscustomobject]@{
    enabled              = [bool]($WriteArtifactProofMatrix -or $FailOnArtifactProofGap)
    status               = "not-run"
    json                 = ""
    markdown             = ""
    command              = ""
    exitCode             = $null
    mutatingCommandCount = $null
    rowsWithRequiredGaps = $null
    gapsByTier           = $null
}

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
        "-OfficeEditSmokeSummaryPath",
        $summaryPath,
        "-OutJson",
        $ArtifactProofMatrixJson,
        "-OutMarkdown",
        $ArtifactProofMatrixMarkdown
    )
    if ($FailOnArtifactProofGap) {
        $matrixArgs += "-FailOnGap"
    }

    $artifactProofMatrix.json = $ArtifactProofMatrixJson
    $artifactProofMatrix.markdown = $ArtifactProofMatrixMarkdown
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
    $summary | Add-Member -NotePropertyName artifactProofMatrix -NotePropertyValue $artifactProofMatrix -Force
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    if ($matrixExitCode -ne 0) {
        exit $matrixExitCode
    }
}
else {
    $summary | Add-Member -NotePropertyName artifactProofMatrix -NotePropertyValue $artifactProofMatrix -Force
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
}

Write-Host ("Office edit smoke checked {0} scenario(s); {1} passed, {2} failed." -f $results.Count, ($results.Count - $failed.Count), $failed.Count)
Write-Host ("Summary: {0}" -f $summaryPath)
if ($artifactProofMatrix.enabled) {
    Write-Host ("Artifact proof matrix: {0}" -f $artifactProofMatrix.json)
}

if ($failed.Count -gt 0 -or $oracleExitCode -ne 0) {
    exit 1
}
