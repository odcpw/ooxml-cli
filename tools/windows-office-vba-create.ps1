[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("xlsx", "xlsm", "excel", "pptx", "pptm", "powerpoint")]
    [string]$Family,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [string[]]$SourcePath = @(),

    [string]$SourcePathJson = "",

    [string]$ExtractBinPath = "",

    [switch]$EnableVbaObjectModelAccess,

    [switch]$Visible,

    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-AbsolutePath {
    param([string]$Path)
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path (Get-Location).Path $Path))
}

function Release-ComObject {
    param([object]$Object)
    if ($null -ne $Object -and [System.Runtime.InteropServices.Marshal]::IsComObject($Object)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Object)
    }
}

function Enable-VBOMAccess {
    param([string[]]$Applications)
    $state = @{}
    foreach ($app in $Applications) {
        $path = "HKCU:\Software\Microsoft\Office\16.0\$app\Security"
        if (-not (Test-Path -LiteralPath $path)) {
            New-Item -Path $path -Force | Out-Null
        }
        $prop = Get-ItemProperty -LiteralPath $path -Name AccessVBOM -ErrorAction SilentlyContinue
        if ($null -eq $prop) {
            $state[$app] = [pscustomobject]@{ path = $path; value = $null; existed = $false }
        }
        else {
            $state[$app] = [pscustomobject]@{ path = $path; value = [int]$prop.AccessVBOM; existed = $true }
        }
        New-ItemProperty -LiteralPath $path -Name AccessVBOM -PropertyType DWord -Value 1 -Force | Out-Null
    }
    return $state
}

function Restore-VBOMAccess {
    param([hashtable]$State)
    foreach ($key in $State.Keys) {
        $entry = $State[$key]
        if ($entry.existed) {
            New-ItemProperty -LiteralPath $entry.path -Name AccessVBOM -PropertyType DWord -Value $entry.value -Force | Out-Null
        }
        else {
            Remove-ItemProperty -LiteralPath $entry.path -Name AccessVBOM -ErrorAction SilentlyContinue
        }
    }
}

function Assert-VBOMAvailable {
    param([object]$Project, [string]$Application)
    if ($null -eq $Project) {
        $hint = "Enable Trust access to the VBA project object model"
        if (-not $EnableVbaObjectModelAccess) {
            $hint += " or rerun with -EnableVbaObjectModelAccess"
        }
        throw "$Application did not expose VBProject through COM. $hint."
    }
}

function Import-VBASources {
    param([object]$Project, [string[]]$Sources)
    $imported = @()
    foreach ($source in $Sources) {
        $component = $Project.VBComponents.Import($source)
        if ($null -eq $component) {
            throw "Office did not import VBA source: $source"
        }
        $kind = ""
        try { $kind = [string]$component.Type } catch {}
        $imported += [pscustomobject]@{
            source = $source
            name   = [string]$component.Name
            type   = $kind
        }
    }
    return $imported
}

function New-ExcelMacroWorkbook {
    param([string]$Path, [string[]]$Sources)
    $excel = $null
    $workbook = $null
    try {
        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = [bool]$Visible
        $excel.DisplayAlerts = $false
        $workbook = $excel.Workbooks.Add()
        $project = $workbook.VBProject
        Assert-VBOMAvailable -Project $project -Application "Excel"
        $imported = Import-VBASources -Project $project -Sources $Sources
        $workbook.SaveAs($Path, 52)
        return $imported
    }
    finally {
        if ($null -ne $workbook) { try { $workbook.Close($false) } catch {} }
        if ($null -ne $excel) { try { $excel.Quit() } catch {} }
        Release-ComObject -Object $workbook
        Release-ComObject -Object $excel
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function New-PowerPointMacroPresentation {
    param([string]$Path, [string[]]$Sources)
    $powerpoint = $null
    $presentation = $null
    try {
        $powerpoint = New-Object -ComObject PowerPoint.Application
        try { $powerpoint.DisplayAlerts = 1 } catch {}
        $presentation = $powerpoint.Presentations.Add([bool]$Visible)
        [void]$presentation.Slides.Add(1, 12)
        $project = $presentation.VBProject
        Assert-VBOMAvailable -Project $project -Application "PowerPoint"
        $imported = Import-VBASources -Project $project -Sources $Sources
        $presentation.SaveAs($Path, 25)
        return $imported
    }
    finally {
        if ($null -ne $presentation) { try { $presentation.Close() } catch {} }
        if ($null -ne $powerpoint) { try { $powerpoint.Quit() } catch {} }
        Release-ComObject -Object $presentation
        Release-ComObject -Object $powerpoint
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function Copy-VBAProjectBin {
    param([string]$PackagePath, [string]$FamilyName, [string]$DestinationPath)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $entryName = if ($FamilyName -eq "xlsx") { "xl/vbaProject.bin" } else { "ppt/vbaProject.bin" }
    $zip = [System.IO.Compression.ZipFile]::OpenRead($PackagePath)
    try {
        $entry = $zip.GetEntry($entryName)
        if ($null -eq $entry) {
            throw "Created package did not contain $entryName"
        }
        $parent = Split-Path -Parent $DestinationPath
        if ($parent -ne "") {
            New-Item -ItemType Directory -Force -Path $parent | Out-Null
        }
        $inputStream = $entry.Open()
        try {
            $outputStream = [System.IO.File]::Create($DestinationPath)
            try {
                $inputStream.CopyTo($outputStream)
            }
            finally {
                $outputStream.Dispose()
            }
        }
        finally {
            $inputStream.Dispose()
        }
    }
    finally {
        $zip.Dispose()
    }
}

function Get-SHA256 {
    param([string]$Path)
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

$familyName = switch ($Family.ToLowerInvariant()) {
    { $_ -in @("xlsx", "xlsm", "excel") } { "xlsx"; break }
    { $_ -in @("pptx", "pptm", "powerpoint") } { "pptx"; break }
}

$output = Resolve-AbsolutePath -Path $OutputPath
$sourceInputs = @()
$jsonSources = @()
if ($SourcePathJson.Trim() -ne "") {
    $decodedSources = ConvertFrom-Json -InputObject $SourcePathJson
    foreach ($source in @($decodedSources)) {
        $trimmed = ([string]$source).Trim()
        if ($trimmed -ne "") {
            $jsonSources += $trimmed
        }
    }
}
if ($jsonSources.Count -gt 0) {
    $sourceInputs = @($jsonSources)
}
else {
    foreach ($source in $SourcePath) {
        $trimmed = ([string]$source).Trim()
        if ($trimmed -eq "") {
            continue
        }
        $resolvedWhole = Resolve-AbsolutePath -Path $trimmed
        if (Test-Path -LiteralPath $resolvedWhole -PathType Leaf) {
            $sourceInputs += $trimmed
            continue
        }
        foreach ($part in ($trimmed -split ",")) {
            $part = $part.Trim()
            if ($part -ne "") {
                $sourceInputs += $part
            }
        }
    }
}
if ($sourceInputs.Count -eq 0) {
    throw "At least one VBA source path is required."
}

$sources = @()
foreach ($source in $sourceInputs) {
    $resolved = Resolve-AbsolutePath -Path $source
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "VBA source file not found: $source"
    }
    $extension = [System.IO.Path]::GetExtension($resolved).ToLowerInvariant()
    if ($extension -notin @(".bas", ".cls")) {
        throw "VBA source must be .bas or .cls: $source"
    }
    $sources += $resolved
}

$expectedExtension = if ($familyName -eq "xlsx") { ".xlsm" } else { ".pptm" }
if ([System.IO.Path]::GetExtension($output).ToLowerInvariant() -ne $expectedExtension) {
    throw "OutputPath for $familyName must end with $expectedExtension"
}

$outputParent = Split-Path -Parent $output
if ($outputParent -ne "") {
    New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
}
if (Test-Path -LiteralPath $output -PathType Leaf) {
    if (-not $Force) {
        throw "OutputPath already exists. Pass -Force to overwrite: $output"
    }
    Remove-Item -LiteralPath $output -Force
}

$binOutput = ""
if ($ExtractBinPath -ne "") {
    $binOutput = Resolve-AbsolutePath -Path $ExtractBinPath
    if (Test-Path -LiteralPath $binOutput -PathType Leaf) {
        if (-not $Force) {
            throw "ExtractBinPath already exists. Pass -Force to overwrite: $binOutput"
        }
        Remove-Item -LiteralPath $binOutput -Force
    }
}

$appsForVBOM = if ($familyName -eq "xlsx") { @("Excel") } else { @("PowerPoint") }
$vbomState = @{}
if ($EnableVbaObjectModelAccess) {
    $vbomState = Enable-VBOMAccess -Applications $appsForVBOM
}

try {
    $imported = if ($familyName -eq "xlsx") {
        New-ExcelMacroWorkbook -Path $output -Sources $sources
    }
    else {
        New-PowerPointMacroPresentation -Path $output -Sources $sources
    }
}
finally {
    if ($EnableVbaObjectModelAccess) {
        Restore-VBOMAccess -State $vbomState
    }
}

if ($binOutput -ne "") {
    Copy-VBAProjectBin -PackagePath $output -FamilyName $familyName -DestinationPath $binOutput
}

$summary = [pscustomobject]@{
    family              = $familyName
    output              = $output
    outputSha256        = Get-SHA256 -Path $output
    vbaProjectBin       = $binOutput
    vbaProjectBinSha256 = if ($binOutput -ne "") { Get-SHA256 -Path $binOutput } else { "" }
    sources             = @($sources)
    importedModules     = @($imported)
    proofLevel          = "microsoft-office-authored"
}

$summary | ConvertTo-Json -Depth 6
