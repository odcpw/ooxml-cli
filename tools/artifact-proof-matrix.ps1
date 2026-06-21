[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path,

    [string]$BinaryPath = "",

    [string]$CapabilitiesJsonPath = "",

    [string]$EvidencePath = "",

    [string[]]$OfficeEditSmokeSummaryPath = @(),

    [string]$OutJson = "",

    [string]$OutMarkdown = "",

    [switch]$FailOnGap
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$SchemaVersion = "ooxml-cli.artifact-proof-matrix.v1"
$TierNames = @("structural", "readback", "validate", "conformance", "office")
$PassingTierStatuses = @("passed", "not-required", "not-applicable", "waived")

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

function Get-JsonObject {
    param([string]$JsonText)

    if ($JsonText.Trim() -eq "") {
        throw "Expected JSON text but received an empty string."
    }
    return ($JsonText | ConvertFrom-Json)
}

function Get-PropertyValue {
    param(
        [object]$Object,
        [string]$Name
    )

    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Resolve-CargoPath {
    $fromPath = Get-Command cargo -ErrorAction SilentlyContinue
    if ($null -ne $fromPath) {
        return $fromPath.Source
    }
    $default = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    if (Test-Path -LiteralPath $default -PathType Leaf) {
        return $default
    }
    throw "cargo was not found. Pass -BinaryPath or -CapabilitiesJsonPath."
}

function Resolve-OoxmlBinary {
    param(
        [string]$Requested,
        [string]$Root
    )

    if ($Requested -ne "") {
        if (-not (Test-Path -LiteralPath $Requested -PathType Leaf)) {
            throw "BinaryPath does not exist: $Requested"
        }
        return (Resolve-Path -LiteralPath $Requested).Path
    }

    $candidate = Join-Path $Root "target\debug\ooxml.exe"
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        return (Resolve-Path -LiteralPath $candidate).Path
    }

    $candidateNoExe = Join-Path $Root "target\debug\ooxml"
    if (Test-Path -LiteralPath $candidateNoExe -PathType Leaf) {
        return (Resolve-Path -LiteralPath $candidateNoExe).Path
    }

    return ""
}

function Get-Capabilities {
    param(
        [string]$Root,
        [string]$Binary,
        [string]$JsonPath
    )

    if ($JsonPath -ne "") {
        if (-not (Test-Path -LiteralPath $JsonPath -PathType Leaf)) {
            throw "CapabilitiesJsonPath does not exist: $JsonPath"
        }
        $text = Get-Content -LiteralPath $JsonPath -Raw
        return @{
            Object = (Get-JsonObject -JsonText $text)
            Source = (Resolve-Path -LiteralPath $JsonPath).Path
            Command = ""
        }
    }

    if ($Binary -ne "") {
        $args = @("--json", "capabilities")
        $output = @(& $Binary @args)
        if ($LASTEXITCODE -ne 0) {
            throw ("capabilities command failed with exit code {0}: {1}" -f $LASTEXITCODE, (Format-CommandLine -FilePath $Binary -Arguments $args))
        }
        return @{
            Object = (Get-JsonObject -JsonText (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine))
            Source = $Binary
            Command = (Format-CommandLine -FilePath $Binary -Arguments $args)
        }
    }

    $cargo = Resolve-CargoPath
    $args = @("run", "--quiet", "--bin", "ooxml", "--", "--json", "capabilities")
    Push-Location -LiteralPath $Root
    try {
        $output = @(& $cargo @args)
        if ($LASTEXITCODE -ne 0) {
            throw ("cargo capabilities command failed with exit code {0}: {1}" -f $LASTEXITCODE, (Format-CommandLine -FilePath $cargo -Arguments $args))
        }
    }
    finally {
        Pop-Location
    }

    return @{
        Object = (Get-JsonObject -JsonText (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine))
        Source = $cargo
        Command = (Format-CommandLine -FilePath $cargo -Arguments $args)
    }
}

function Get-FlagNames {
    param([object]$Command)

    $flags = @(Get-PropertyValue -Object $Command -Name "localFlags")
    return @($flags | ForEach-Object {
        $name = Get-PropertyValue -Object $_ -Name "name"
        if ($null -ne $name) {
            [string]$name
        }
    })
}

function Test-HasAnyFlag {
    param(
        [string[]]$FlagNames,
        [string[]]$Needles
    )

    foreach ($needle in $Needles) {
        if ($FlagNames -contains $needle) {
            return $true
        }
    }
    return $false
}

function Get-OutputFamily {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $use = [string](Get-PropertyValue -Object $Command -Name "use")
    $text = "$path $use".ToLowerInvariant()

    if ($path -match '^ooxml vba(\s|$)') {
        return "macro"
    }
    if ($path -eq "ooxml apply" -or $path -eq "ooxml find" -or $path -eq "ooxml template apply") {
        return "package"
    }
    if ($text -match '\bdocx\b|\.docx|\.docm') {
        return "docx"
    }
    if ($text -match '\bpptx\b|\.pptx|\.pptm') {
        return "pptx"
    }
    if ($text -match '\bxlsx\b|\.xlsx|\.xlsm') {
        return "xlsx"
    }
    return "package"
}

function Get-ArtifactExtension {
    param(
        [string]$Family,
        [string]$Path
    )

    if ($Path -eq "ooxml vba create") {
        return "xlsm"
    }
    switch ($Family) {
        "docx" { return "docx" }
        "macro" { return "xlsm" }
        "pptx" { return "pptx" }
        "xlsx" { return "xlsx" }
        default { return "ooxml" }
    }
}

function ConvertTo-Slug {
    param([string]$Path)

    $slug = $Path.ToLowerInvariant() -replace '^ooxml\s+', ''
    $slug = $slug -replace '[^a-z0-9]+', '-'
    $slug = $slug.Trim("-")
    if ($slug -eq "") {
        return "command"
    }
    return $slug
}

function Get-InputFixtureType {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $use = [string](Get-PropertyValue -Object $Command -Name "use")
    $reason = [string](Get-PropertyValue -Object $Command -Name "opIneligibleReason")
    $text = "$path $use $reason".ToLowerInvariant()

    if ($path -match '^ooxml (docx|xlsx|pptx) scaffold$') {
        return "scaffold"
    }
    if ($path -eq "ooxml vba create") {
        return "source modules"
    }
    if ($path -eq "ooxml pptx template compile") {
        return "template manifest/spec"
    }
    if ($text -match 'cross-package|--source|<source-file>|<target-file>|--workbook') {
        return "realistic fixture"
    }
    if ($text -match 'damaged|corrupt|repair') {
        return "intentionally damaged file"
    }
    return "clean fixture"
}

function Test-PublicPackageMutator {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $flagNames = @(Get-FlagNames -Command $Command)
    $hasPackageWriteFlags = (Test-HasAnyFlag -FlagNames $flagNames -Needles @("--out", "--in-place")) -and (Test-HasAnyFlag -FlagNames $flagNames -Needles @("--dry-run", "--in-place"))
    $opCompatible = [bool](Get-PropertyValue -Object $Command -Name "opCompatible")

    if ($path -match '^ooxml (docx|xlsx|pptx) scaffold$') {
        return $true
    }
    if ($path -eq "ooxml vba create" -or $path -eq "ooxml pptx template compile") {
        return $true
    }
    if ($path -eq "ooxml apply" -or $path -eq "ooxml template apply") {
        return $true
    }
    if ($path -eq "ooxml find" -and ($flagNames -contains "--apply")) {
        return $true
    }
    if ($opCompatible -and (Test-HasAnyFlag -FlagNames $flagNames -Needles @("--out", "--in-place"))) {
        return $true
    }
    if ($hasPackageWriteFlags) {
        return $true
    }
    return $false
}

function Get-CommandTailFromUse {
    param(
        [string]$Path,
        [string]$Use
    )

    $candidate = $Use.Trim()
    $pathTail = ($Path -replace '^ooxml\s+', '').Trim()
    if ($candidate -eq $pathTail) {
        return ""
    }
    if ($candidate.StartsWith($pathTail + " ")) {
        return $candidate.Substring($pathTail.Length).TrimStart()
    }

    $pathParts = @($Path -split '\s+' | Where-Object { $_ -ne "" })
    $last = $pathParts[$pathParts.Count - 1]
    if ($candidate -eq $last) {
        return ""
    }
    if ($candidate.StartsWith($last + " ")) {
        return $candidate.Substring($last.Length).TrimStart()
    }
    return $candidate
}

function Get-CliCommandTemplate {
    param(
        [object]$Command,
        [string]$ArtifactPath
    )

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $use = [string](Get-PropertyValue -Object $Command -Name "use")
    $flagNames = @(Get-FlagNames -Command $Command)
    $tail = Get-CommandTailFromUse -Path $path -Use $use
    if (($flagNames -contains "--out") -and $tail -notmatch '--out\b') {
        $tail = ($tail + " --out <$ArtifactPath>").Trim()
    }

    $pathTail = ($path -replace '^ooxml\s+', '').Trim()
    if ($tail -eq "") {
        return "ooxml --json $pathTail"
    }
    return "ooxml --json $pathTail $tail"
}

function Get-MutationKind {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    if ($path -match '^ooxml (docx|xlsx|pptx) scaffold$') {
        return "create-package"
    }
    if ($path -eq "ooxml vba create" -or $path -eq "ooxml pptx template compile") {
        return "create-package"
    }
    if ($path -eq "ooxml apply") {
        return "batch-package-mutation"
    }
    if ($path -eq "ooxml find") {
        return "conditional-apply-mutation"
    }
    return "package-mutation"
}

function Get-StructuralAssertion {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $kinds = @((Get-PropertyValue -Object $Command -Name "targetObjectKinds") | ForEach-Object { [string]$_ })
    $text = ("$path " + ($kinds -join " ")).ToLowerInvariant()

    if ($text -match 'vba|module') {
        return "Check macro content types, relationships, and vbaProject.bin wiring."
    }
    if ($path -match '^ooxml docx(\s|$)') {
        if ($text -match 'comment') {
            return "Check word/comments.xml, comment relationships, and the anchored document ranges."
        }
        if ($text -match 'header|footer') {
            return "Check word/header*.xml or word/footer*.xml plus document relationships and section references."
        }
        if ($text -match 'field') {
            return "Check word/document.xml field runs, field instructions, and result text."
        }
        if ($text -match 'image') {
            return "Check word/document.xml drawing markup, media parts, relationships, and content types."
        }
        return "Check word/document.xml body structure, paragraph/table order, styles, and section properties."
    }
    if ($path -match '^ooxml pptx(\s|$)') {
        if ($text -match 'chart') {
            return "Check chart part XML, chart relationships, embedded workbook links, and content types."
        }
        if ($text -match 'table') {
            return "Check slide table XML, row/column counts, merged-cell state, and slide relationships."
        }
        return "Check ppt/presentation.xml, slide/layout/master XML, relationships, content types, and affected media."
    }
    if ($text -match 'chart') {
        return "Check chart part XML, chart relationships, embedded workbook links, and content types."
    }
    if ($text -match 'pivot') {
        return "Check workbook pivot cache definitions, pivot table parts, records, and relationships."
    }
    if ($text -match 'table') {
        return "Check table XML refs/counts, table relationships, and the host sheet/table references."
    }
    if ($text -match 'conditional-format') {
        return "Check worksheet conditionalFormatting order, sqref, rule priority, and rule XML."
    }
    if ($text -match 'data-validation') {
        return "Check worksheet dataValidations count, sqref, and child order."
    }
    if ($text -match 'name') {
        return "Check xl/workbook.xml definedNames ordering and definedName scope/ref attributes."
    }
    if ($text -match 'sheet|range|cell|hyperlink|comment|freeze|row|col') {
        return "Check workbook/sheet XML child order plus affected worksheet cells, dimensions, rels, or comments."
    }
    return "Check package relationships, content types, and the primary mutated OOXML part."
}

function Get-ReadbackCommandTemplate {
    param(
        [object]$Command,
        [string]$ArtifactPath
    )

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $kinds = @((Get-PropertyValue -Object $Command -Name "targetObjectKinds") | ForEach-Object { [string]$_ })
    $text = ("$path " + ($kinds -join " ")).ToLowerInvariant()
    $artifact = "<$ArtifactPath>"

    if ($text -match 'vba|module') {
        return "ooxml --json vba inspect $artifact"
    }
    if ($path -match '^ooxml docx(\s|$)') {
        if ($text -match 'comment') { return "ooxml --json docx comments list $artifact" }
        if ($text -match 'field') { return "ooxml --json docx fields list $artifact" }
        if ($text -match 'header') { return "ooxml --json docx headers list $artifact" }
        if ($text -match 'footer') { return "ooxml --json docx footers list $artifact" }
        if ($text -match 'image') { return "ooxml --json docx images list $artifact" }
        if ($text -match 'table' -and $path -match 'docx tables') { return "ooxml --json docx tables show $artifact --table 1" }
        return "ooxml --json docx blocks $artifact"
    }
    if ($path -match '^ooxml pptx(\s|$)') {
        if ($text -match 'chart') { return "ooxml --json pptx charts list $artifact --slide 1" }
        if ($text -match 'comment') { return "ooxml --json pptx comments list $artifact --slide 1" }
        if ($text -match 'table') { return "ooxml --json pptx tables show $artifact --slide 1 --table 1" }
        if ($text -match 'field') { return "ooxml --json pptx fields inspect $artifact" }
        if ($text -match 'media|image') { return "ooxml --json pptx media list $artifact" }
        if ($text -match 'layout') { return "ooxml --json pptx layouts list $artifact" }
        if ($text -match 'master') { return "ooxml --json pptx masters list $artifact" }
        return "ooxml --json pptx slides list $artifact"
    }
    if ($text -match 'chart') {
        return "ooxml --json xlsx charts list $artifact"
    }
    if ($text -match 'pivot') {
        return "ooxml --json xlsx pivots list $artifact"
    }
    if ($text -match 'conditional-format') {
        return "ooxml --json xlsx conditional-formats list $artifact --sheet <selector>"
    }
    if ($text -match 'data-validation') {
        return "ooxml --json xlsx data-validations list $artifact --sheet <selector>"
    }
    if ($text -match 'hyperlink') {
        return "ooxml --json xlsx hyperlinks list $artifact --sheet <selector>"
    }
    if ($text -match 'comment') {
        return "ooxml --json xlsx comments list $artifact --sheet <selector>"
    }
    if ($text -match 'table') {
        return "ooxml --json xlsx tables list $artifact"
    }
    if ($text -match 'name') {
        return "ooxml --json xlsx names list $artifact"
    }
    if ($text -match 'sheet') {
        return "ooxml --json xlsx sheets list $artifact"
    }
    if ($text -match 'range|cell') {
        return "ooxml --json xlsx ranges export $artifact --sheet <selector> --range <range> --include-types --include-formulas"
    }
    return "ooxml --json inspect $artifact"
}

function Test-SnapshotRecommended {
    param([object]$Command)

    $path = [string](Get-PropertyValue -Object $Command -Name "path")
    $kinds = @((Get-PropertyValue -Object $Command -Name "targetObjectKinds") | ForEach-Object { [string]$_ })
    $text = ("$path " + ($kinds -join " ")).ToLowerInvariant()
    return ($text -match 'chart|table|pivot|conditional-format|data-validation|template|xlsx-bindings|module|media|image|animation')
}

function New-TierStatus {
    param(
        [string]$Name,
        [bool]$Required,
        [string]$DefaultStatus,
        [string]$Detail
    )

    return [ordered]@{
        name = $Name
        required = $Required
        status = $DefaultStatus
        detail = $Detail
        evidence = @()
    }
}

function Merge-TierEvidence {
    param(
        [System.Collections.IDictionary]$Tier,
        [object]$EvidenceTier
    )

    if ($null -eq $EvidenceTier) {
        return
    }
    if ($EvidenceTier -is [string]) {
        $Tier.status = [string]$EvidenceTier
        return
    }

    $status = Get-PropertyValue -Object $EvidenceTier -Name "status"
    if ($null -ne $status) {
        $Tier.status = [string]$status
    }
    $detail = Get-PropertyValue -Object $EvidenceTier -Name "detail"
    if ($null -ne $detail) {
        $Tier.detail = [string]$detail
    }
    $evidence = Get-PropertyValue -Object $EvidenceTier -Name "evidence"
    if ($null -ne $evidence) {
        $Tier.evidence = @($evidence | ForEach-Object { [string]$_ })
    }
}

function Test-TierGap {
    param([System.Collections.IDictionary]$Tier)

    if (-not [bool]$Tier.required) {
        return $false
    }
    return -not ($PassingTierStatuses -contains [string]$Tier.status)
}

function Get-HighestEvidenceTier {
    param([hashtable]$Tiers)

    $ranked = @("office", "conformance", "validate", "readback", "structural")
    foreach ($name in $ranked) {
        $tier = $Tiers[$name]
        if ($null -ne $tier -and ($PassingTierStatuses -contains [string]$tier.status)) {
            return $name
        }
    }
    return "none"
}

function Import-Evidence {
    param([string]$Path)

    $map = @{}
    if ($Path -eq "") {
        return $map
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "EvidencePath does not exist: $Path"
    }
    $doc = Get-JsonObject -JsonText (Get-Content -LiteralPath $Path -Raw)
    $items = Get-PropertyValue -Object $doc -Name "proofs"
    if ($null -eq $items) {
        $items = Get-PropertyValue -Object $doc -Name "rows"
    }
    foreach ($item in @($items)) {
        $commandPath = Get-PropertyValue -Object $item -Name "commandPath"
        if ($null -ne $commandPath -and [string]$commandPath -ne "") {
            $map[[string]$commandPath] = $item
        }
    }
    return $map
}

function ConvertTo-OrderedPropertyMap {
    param([object]$Object)

    $map = [ordered]@{}
    if ($null -eq $Object) {
        return $map
    }
    foreach ($property in @($Object.PSObject.Properties)) {
        $map[$property.Name] = $property.Value
    }
    return $map
}

function Merge-EvidenceItem {
    param(
        [object]$Existing,
        [object]$Incoming
    )

    if ($null -eq $Existing) {
        return $Incoming
    }
    if ($null -eq $Incoming) {
        return $Existing
    }

    $result = ConvertTo-OrderedPropertyMap -Object $Existing
    foreach ($property in @($Incoming.PSObject.Properties)) {
        if ($property.Name -eq "tiers") {
            continue
        }
        $current = $result[$property.Name]
        if ($null -eq $current -or [string]$current -eq "") {
            $result[$property.Name] = $property.Value
        }
    }

    $mergedTiers = ConvertTo-OrderedPropertyMap -Object (Get-PropertyValue -Object $Existing -Name "tiers")
    $incomingTiers = Get-PropertyValue -Object $Incoming -Name "tiers"
    foreach ($tierName in $TierNames) {
        $incomingTier = Get-PropertyValue -Object $incomingTiers -Name $tierName
        if ($null -eq $incomingTier) {
            continue
        }
        $existingTier = Get-PropertyValue -Object ([pscustomobject]$mergedTiers) -Name $tierName
        $existingStatus = Get-PropertyValue -Object $existingTier -Name "status"
        $incomingStatus = Get-PropertyValue -Object $incomingTier -Name "status"
        if ($null -eq $existingTier -or -not ($PassingTierStatuses -contains [string]$existingStatus) -or ($PassingTierStatuses -contains [string]$incomingStatus)) {
            $mergedTiers[$tierName] = $incomingTier
        }
    }
    $result["tiers"] = [pscustomobject]$mergedTiers
    return [pscustomobject]$result
}

function Merge-EvidenceMaps {
    param(
        [hashtable]$Target,
        [hashtable]$Incoming
    )

    foreach ($key in $Incoming.Keys) {
        if ($Target.ContainsKey($key)) {
            $Target[$key] = Merge-EvidenceItem -Existing $Target[$key] -Incoming $Incoming[$key]
        }
        else {
            $Target[$key] = $Incoming[$key]
        }
    }
}

function Find-CommandPathInSmokeCommand {
    param(
        [string]$CommandLine,
        [string[]]$CommandPaths
    )

    if ($CommandLine -eq "") {
        return ""
    }
    $normalized = " " + (($CommandLine -replace "[`r`n]+", " ") -replace "\s+", " ") + " "
    foreach ($path in @($CommandPaths | Sort-Object Length -Descending)) {
        $tail = ($path -replace "^ooxml\s+", "").Trim()
        if ($tail -eq "") {
            continue
        }
        $pattern = "(?i)(?:^|\s)--json\s+(?:--strict\s+)?" + [regex]::Escape($tail) + "(?:\s|$)"
        if ($normalized -match $pattern) {
            return $path
        }
    }
    return ""
}

function New-SmokeTierEvidence {
    param(
        [object]$Stage,
        [string]$PassedDetail,
        [string]$FallbackDetail,
        [string[]]$ExtraEvidence = @()
    )

    if ($null -eq $Stage) {
        return $null
    }
    $status = [string](Get-PropertyValue -Object $Stage -Name "status")
    if ($status -eq "" -or $status -eq "not-run" -or $status -eq "skipped") {
        return $null
    }
    $detail = [string](Get-PropertyValue -Object $Stage -Name "detail")
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
        $value = Get-PropertyValue -Object $Stage -Name $name
        if ($null -ne $value -and [string]$value -ne "") {
            [void]$evidence.Add([string]$value)
        }
    }
    return [pscustomobject][ordered]@{
        status = $status
        detail = $detail
        evidence = @($evidence)
    }
}

function Import-OfficeEditSmokeEvidence {
    param(
        [string[]]$Paths,
        [string[]]$CommandPaths
    )

    $map = @{}
    foreach ($path in @($Paths | Where-Object { $null -ne $_ -and $_ -ne "" })) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "OfficeEditSmokeSummaryPath does not exist: $path"
        }
        $resolved = (Resolve-Path -LiteralPath $path).Path
        $doc = Get-JsonObject -JsonText (Get-Content -LiteralPath $resolved -Raw)
        foreach ($scenario in @((Get-PropertyValue -Object $doc -Name "scenarios"))) {
            $mutation = Get-PropertyValue -Object $scenario -Name "mutation"
            $commandLine = [string](Get-PropertyValue -Object $mutation -Name "command")
            $commandPath = Find-CommandPathInSmokeCommand -CommandLine $commandLine -CommandPaths $CommandPaths
            if ($commandPath -eq "") {
                continue
            }

            $scenarioName = [string](Get-PropertyValue -Object $scenario -Name "name")
            $output = [string](Get-PropertyValue -Object $scenario -Name "output")
            $scenarioEvidence = @("office-edit-smoke summary: $resolved", "scenario: $scenarioName", "output: $output")
            $tiers = [ordered]@{}
            $tiers.readback = New-SmokeTierEvidence `
                -Stage (Get-PropertyValue -Object $scenario -Name "readback") `
                -PassedDetail "ooxml inspect read the saved smoke output and reported the expected Office family." `
                -FallbackDetail "ooxml inspect did not read the saved smoke output cleanly." `
                -ExtraEvidence $scenarioEvidence
            $tiers.structural = New-SmokeTierEvidence `
                -Stage (Get-PropertyValue -Object $scenario -Name "openXmlSdk") `
                -PassedDetail "Microsoft Open XML SDK schema validation passed for the smoke output." `
                -FallbackDetail "Microsoft Open XML SDK schema validation did not pass for the smoke output." `
                -ExtraEvidence $scenarioEvidence
            $tiers.validate = New-SmokeTierEvidence `
                -Stage (Get-PropertyValue -Object $scenario -Name "validation") `
                -PassedDetail "ooxml validate --strict accepted the smoke output." `
                -FallbackDetail "ooxml validate --strict did not accept the smoke output." `
                -ExtraEvidence $scenarioEvidence
            $tiers.conformance = New-SmokeTierEvidence `
                -Stage (Get-PropertyValue -Object $scenario -Name "conformance") `
                -PassedDetail "ooxml conformance check accepted the smoke output." `
                -FallbackDetail "ooxml conformance check did not accept the smoke output." `
                -ExtraEvidence $scenarioEvidence
            $tiers.office = New-SmokeTierEvidence `
                -Stage (Get-PropertyValue -Object $scenario -Name "microsoftOffice") `
                -PassedDetail "Desktop Microsoft Office opened the smoke output without repair/failure." `
                -FallbackDetail "Desktop Microsoft Office did not open the smoke output cleanly." `
                -ExtraEvidence $scenarioEvidence

            foreach ($tierName in @($TierNames)) {
                if ($null -eq $tiers[$tierName]) {
                    $tiers.Remove($tierName)
                }
            }
            $fixtureType = "office edit smoke fixture"
            if ($commandPath -match '^ooxml (docx|xlsx|pptx) scaffold$') {
                $fixtureType = "scaffold"
            }
            $item = [pscustomobject][ordered]@{
                commandPath = $commandPath
                inputFixtureType = $fixtureType
                generatedOutputPath = $output
                exactCommand = $commandLine
                sourceSummary = $resolved
                scenarioName = $scenarioName
                tiers = [pscustomobject]$tiers
            }
            if ($map.ContainsKey($commandPath)) {
                $map[$commandPath] = Merge-EvidenceItem -Existing $map[$commandPath] -Incoming $item
            }
            else {
                $map[$commandPath] = $item
            }
        }
    }
    return $map
}

function Escape-MarkdownCell {
    param([object]$Value)

    if ($null -eq $Value) {
        return ""
    }
    $text = [string]$Value
    $text = $text -replace '\|', '\|'
    $text = $text -replace "`r?`n", "<br>"
    return $text
}

function Write-MarkdownReport {
    param(
        [object]$Matrix,
        [string]$Path
    )

    $lines = New-Object System.Collections.Generic.List[string]
    [void]$lines.Add("# OOXML CLI Artifact Proof Matrix")
    [void]$lines.Add("")
    [void]$lines.Add(("Generated: {0}" -f $Matrix.generatedAtUtc))
    [void]$lines.Add("")
    [void]$lines.Add(("Mutating commands: {0}" -f $Matrix.summary.mutatingCommandCount))
    [void]$lines.Add(("Rows with required proof gaps: {0}" -f $Matrix.summary.rowsWithRequiredGaps))
    [void]$lines.Add("")
    [void]$lines.Add("| tier | gaps |")
    [void]$lines.Add("| --- | ---: |")
    foreach ($tier in $TierNames) {
        [void]$lines.Add(("| {0} | {1} |" -f $tier, $Matrix.summary.gapsByTier.$tier))
    }
    [void]$lines.Add("")
    [void]$lines.Add("| family | command | fixture | artifact | structural | readback | validate | conformance | office |")
    [void]$lines.Add("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    foreach ($row in @($Matrix.rows)) {
        [void]$lines.Add((
            "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} |" -f
            (Escape-MarkdownCell $row.outputFamily),
            (Escape-MarkdownCell $row.commandPath),
            (Escape-MarkdownCell $row.inputFixtureType),
            (Escape-MarkdownCell $row.generatedOutputPath),
            (Escape-MarkdownCell $row.tiers.structural.status),
            (Escape-MarkdownCell $row.tiers.readback.status),
            (Escape-MarkdownCell $row.tiers.validate.status),
            (Escape-MarkdownCell $row.tiers.conformance.status),
            (Escape-MarkdownCell $row.tiers.office.status)
        ))
    }
    $directory = Split-Path -Parent $Path
    if ($directory -ne "" -and -not (Test-Path -LiteralPath $directory -PathType Container)) {
        New-Item -ItemType Directory -Path $directory | Out-Null
    }
    Set-Content -LiteralPath $Path -Value $lines -Encoding UTF8
}

$resolvedRepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
if ($OutJson -eq "") {
    $OutJson = Join-Path $resolvedRepoRoot ".tmp\artifact-proof-matrix\matrix.json"
}
if ($OutMarkdown -eq "") {
    $OutMarkdown = Join-Path $resolvedRepoRoot ".tmp\artifact-proof-matrix\matrix.md"
}

$resolvedBinary = Resolve-OoxmlBinary -Requested $BinaryPath -Root $resolvedRepoRoot
$capabilitiesResult = Get-Capabilities -Root $resolvedRepoRoot -Binary $resolvedBinary -JsonPath $CapabilitiesJsonPath
$capabilities = $capabilitiesResult.Object
$capabilityCommandPaths = @($capabilities.commands | ForEach-Object { [string](Get-PropertyValue -Object $_ -Name "path") })
$evidenceMap = Import-Evidence -Path $EvidencePath
$officeEditSmokeEvidence = Import-OfficeEditSmokeEvidence -Paths $OfficeEditSmokeSummaryPath -CommandPaths $capabilityCommandPaths
Merge-EvidenceMaps -Target $evidenceMap -Incoming $officeEditSmokeEvidence
$resolvedOfficeEditSmokeSummaryPaths = @($OfficeEditSmokeSummaryPath | Where-Object { $null -ne $_ -and $_ -ne "" } | ForEach-Object { (Resolve-Path -LiteralPath $_).Path })

$rows = New-Object System.Collections.Generic.List[object]
foreach ($command in @($capabilities.commands)) {
    if (-not (Test-PublicPackageMutator -Command $command)) {
        continue
    }

    $path = [string](Get-PropertyValue -Object $command -Name "path")
    $family = Get-OutputFamily -Command $command
    $slug = ConvertTo-Slug -Path $path
    $extension = Get-ArtifactExtension -Family $family -Path $path
    $artifactPath = "proof-artifacts/$slug.$extension"
    if ($family -eq "package") {
        $artifactPath = "proof-artifacts/$slug.<docx|xlsx|pptx>"
    }
    $evidence = $evidenceMap[$path]

    $inputFixtureType = Get-InputFixtureType -Command $command
    $evidenceFixture = Get-PropertyValue -Object $evidence -Name "inputFixtureType"
    if ($null -ne $evidenceFixture) {
        $inputFixtureType = [string]$evidenceFixture
    }

    $evidenceArtifact = Get-PropertyValue -Object $evidence -Name "generatedOutputPath"
    if ($null -ne $evidenceArtifact) {
        $artifactPath = [string]$evidenceArtifact
    }

    $tiers = @{}
    $tiers.structural = New-TierStatus -Name "structural" -Required $true -DefaultStatus "missing" -Detail "OOXML part assertions are required for every package write."
    $tiers.readback = New-TierStatus -Name "readback" -Required $true -DefaultStatus "missing" -Detail "Saved output should be read back through the CLI."
    $tiers.validate = New-TierStatus -Name "validate" -Required $true -DefaultStatus "missing" -Detail "Run ooxml validate --strict on the generated package."
    $tiers.conformance = New-TierStatus -Name "conformance" -Required $true -DefaultStatus "missing" -Detail "Run ooxml --json conformance check on the generated package."
    $tiers.office = New-TierStatus -Name "office" -Required $true -DefaultStatus "missing" -Detail "Representative rows need desktop Office open proof on Windows."

    $evidenceTiers = Get-PropertyValue -Object $evidence -Name "tiers"
    if ($null -eq $evidenceTiers) {
        $evidenceTiers = Get-PropertyValue -Object $evidence -Name "proof"
    }
    foreach ($tierName in $TierNames) {
        Merge-TierEvidence -Tier $tiers[$tierName] -EvidenceTier (Get-PropertyValue -Object $evidenceTiers -Name $tierName)
    }

    $requiredGaps = @()
    foreach ($tierName in $TierNames) {
        if (Test-TierGap -Tier $tiers[$tierName]) {
            $requiredGaps += $tierName
        }
    }

    $exactCommand = Get-PropertyValue -Object $evidence -Name "exactCommand"
    $row = [ordered]@{
        commandPath = $path
        use = [string](Get-PropertyValue -Object $command -Name "use")
        short = [string](Get-PropertyValue -Object $command -Name "short")
        mutationKind = Get-MutationKind -Command $command
        outputFamily = $family
        inputFixtureType = $inputFixtureType
        generatedOutputPath = $artifactPath
        cliCommandTemplate = Get-CliCommandTemplate -Command $command -ArtifactPath $artifactPath
        exactCommand = if ($null -ne $exactCommand) { [string]$exactCommand } else { $null }
        structuralAssertion = Get-StructuralAssertion -Command $command
        readbackCommandTemplate = Get-ReadbackCommandTemplate -Command $command -ArtifactPath $artifactPath
        validateCommandTemplate = "ooxml validate --strict <$artifactPath>"
        conformanceCommandTemplate = "ooxml --json conformance check <$artifactPath>"
        officeProofCommandTemplate = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-oracle.ps1 -RepoRoot . -InputFile <$artifactPath>"
        semanticSnapshotRecommended = [bool](Test-SnapshotRecommended -Command $command)
        localFlags = @(Get-FlagNames -Command $command)
        opCompatible = [bool](Get-PropertyValue -Object $command -Name "opCompatible")
        opIneligibleReason = Get-PropertyValue -Object $command -Name "opIneligibleReason"
        tiers = $tiers
        highestEvidenceTier = Get-HighestEvidenceTier -Tiers $tiers
        requiredGaps = $requiredGaps
    }
    [void]$rows.Add([pscustomobject]$row)
}

$sortedRows = @($rows | Sort-Object outputFamily, commandPath)

$gapsByTier = [ordered]@{}
foreach ($tierName in $TierNames) {
    $gapsByTier[$tierName] = @($sortedRows | Where-Object { $_.requiredGaps -contains $tierName }).Count
}

$byFamily = [ordered]@{}
foreach ($row in $sortedRows) {
    if (-not $byFamily.Contains($row.outputFamily)) {
        $byFamily[$row.outputFamily] = 0
    }
    $byFamily[$row.outputFamily] += 1
}

$rowsWithGaps = @($sortedRows | Where-Object { @($_.requiredGaps).Count -gt 0 })
$structuralProven = @($sortedRows | Where-Object { $_.tiers.structural.status -eq "passed" })
$readbackProven = @($sortedRows | Where-Object { $_.tiers.readback.status -eq "passed" })
$validateProven = @($sortedRows | Where-Object { $_.tiers.validate.status -eq "passed" })
$conformanceProven = @($sortedRows | Where-Object { $_.tiers.conformance.status -eq "passed" })
$officeProven = @($sortedRows | Where-Object { $_.tiers.office.status -eq "passed" })
$scaffoldProven = @($sortedRows | Where-Object { $_.inputFixtureType -eq "scaffold" -and @($_.requiredGaps).Count -eq 0 })
$parserOnly = @($sortedRows | Where-Object { $_.highestEvidenceTier -eq "structural" })

$matrix = [pscustomobject][ordered]@{
    schemaVersion = $SchemaVersion
    generatedAtUtc = [DateTime]::UtcNow.ToString("o")
    source = [ordered]@{
        repoRoot = $resolvedRepoRoot
        capabilitiesSource = $capabilitiesResult.Source
        capabilitiesCommand = $capabilitiesResult.Command
        evidencePath = if ($EvidencePath -ne "") { (Resolve-Path -LiteralPath $EvidencePath).Path } else { "" }
        officeEditSmokeSummaryPaths = $resolvedOfficeEditSmokeSummaryPaths
    }
    policy = [ordered]@{
        rowSource = "Public package-creating and package-mutating commands inferred from ooxml --json capabilities."
        requiredTiers = $TierNames
        passingTierStatuses = $PassingTierStatuses
        note = "Missing means no proof evidence was provided to this matrix run; pass -EvidencePath to overlay checked proof rows."
    }
    summary = [ordered]@{
        totalCapabilityCommands = @($capabilities.commands).Count
        mutatingCommandCount = $sortedRows.Count
        rowsWithRequiredGaps = $rowsWithGaps.Count
        gapsByTier = $gapsByTier
        mutatingCommandsByFamily = $byFamily
        officeEditSmokeEvidenceCommandCount = $officeEditSmokeEvidence.Count
        structuralProvenCommandCount = $structuralProven.Count
        readbackProvenCommandCount = $readbackProven.Count
        validateProvenCommandCount = $validateProven.Count
        conformanceProvenCommandCount = $conformanceProven.Count
        officeProvenCommandCount = $officeProven.Count
        scaffoldProvenCommandCount = $scaffoldProven.Count
        parserOnlyCommandCount = $parserOnly.Count
    }
    questions = [ordered]@{
        createOrMutateCommands = @($sortedRows | ForEach-Object { $_.commandPath })
        scaffoldProvenCommands = @($scaffoldProven | ForEach-Object { $_.commandPath })
        realisticFixtureCommands = @($sortedRows | Where-Object { $_.inputFixtureType -eq "realistic fixture" } | ForEach-Object { $_.commandPath })
        officeProvenCommands = @($officeProven | ForEach-Object { $_.commandPath })
        parserOnlyCommands = @($parserOnly | ForEach-Object { $_.commandPath })
    }
    rows = $sortedRows
}

$jsonDirectory = Split-Path -Parent $OutJson
if ($jsonDirectory -ne "" -and -not (Test-Path -LiteralPath $jsonDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $jsonDirectory | Out-Null
}
$matrix | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $OutJson -Encoding UTF8
Write-MarkdownReport -Matrix $matrix -Path $OutMarkdown

Write-Host ("Artifact proof matrix wrote JSON: {0}" -f $OutJson)
Write-Host ("Artifact proof matrix wrote Markdown: {0}" -f $OutMarkdown)
Write-Host ("Mutating commands: {0}; rows with required gaps: {1}" -f $matrix.summary.mutatingCommandCount, $matrix.summary.rowsWithRequiredGaps)
foreach ($tierName in $TierNames) {
    Write-Host ("  {0}: {1} gaps" -f $tierName, $matrix.summary.gapsByTier.$tierName)
}

if ($FailOnGap -and $matrix.summary.rowsWithRequiredGaps -gt 0) {
    exit 2
}
