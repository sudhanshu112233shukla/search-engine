param(
  [string]$WorkRoot = ""
)

$ErrorActionPreference = "Stop"

# Build a compact "core" offline pack focused on high-value knowledge.
# Strategy:
# - Prefer pre-summarized wiki core data.
# - Optionally add bounded slices of OSM/WARC for breadth.
# - Keep shard sizes reasonable for mobile download/install.

$root = Split-Path -Parent $PSScriptRoot
$rust = Join-Path $root "search_engine_rust"
$data = Join-Path $rust "data"

# Optional external work root (for low C: disk scenarios), e.g. H:\search-engine-work
if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
  $workData = $data
} else {
  $workData = Join-Path $WorkRoot "data"
}

$indexDir = Join-Path $workData "index"
$datasetOut = Join-Path $indexDir "dataset_core.jsonl"

New-Item -ItemType Directory -Force -Path $indexDir | Out-Null

function Sample-Jsonl([string]$inPath, [string]$outPath, [int]$n) {
  if (!(Test-Path $inPath)) { return $false }
  Write-Host "Sample $n lines: $inPath -> $outPath"
  Get-Content $inPath -TotalCount $n | Set-Content $outPath
  return $true
}

function Invoke-CargoChecked {
  param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CmdArgs
  )
  & cargo @CmdArgs
  if ($LASTEXITCODE -ne 0) {
    throw "Cargo command failed: cargo $($CmdArgs -join ' ')"
  }
}

$sources = @()

# Core: summary-focused Wikipedia.
$wikiCore = Join-Path $data "wiki_core.jsonl"
$wikiCoreSample = Join-Path $workData "wiki_core_100k.jsonl"
if (!(Sample-Jsonl $wikiCore $wikiCoreSample 100000)) {
  throw "Missing core source: $wikiCore. Run import-wiki-core first."
}
$sources += $wikiCoreSample

# Optional lightweight breadth packs.
$osmIndia = Join-Path $data "osm_india.jsonl"
$osmIndiaSample = Join-Path $workData "osm_india_core_10000.jsonl"
if (Sample-Jsonl $osmIndia $osmIndiaSample 10000) { $sources += $osmIndiaSample }

$warc = Join-Path $data "warc.jsonl"
$warcSample = Join-Path $workData "warc_core_8000.jsonl"
if (Sample-Jsonl $warc $warcSample 8000) { $sources += $warcSample }

Write-Host "Merging -> $datasetOut"
if (Test-Path $datasetOut) { Remove-Item $datasetOut -Force }
$cmd = "copy /b " + (($sources | ForEach-Object { '"' + $_ + '"' }) -join "+") + " " + ('"' + $datasetOut + '"')
cmd /c $cmd | Out-Null
if ($LASTEXITCODE -ne 0 -or !(Test-Path $datasetOut)) {
  throw "Failed to merge dataset into $datasetOut (check free disk space)."
}
if ((Get-Item $datasetOut).Length -le 0) {
  throw "Merged dataset is empty: $datasetOut"
}

Push-Location $rust
try {
  $packsOut = Join-Path $workData "packs_core"
  $distOut = Join-Path $workData "dist\packs_core"

  Write-Host "Building core packs -> $packsOut"
  Invoke-CargoChecked run --release -- pack --dataset $datasetOut --out $packsOut --max-docs 25000 --lang en

  Write-Host "Validating core packs"
  Invoke-CargoChecked run --release -- validate-pack --dir (Join-Path $packsOut "en") --smoke-query "what is earth"

  Write-Host "Exporting core pack shards -> $distOut"
  Invoke-CargoChecked run --release -- export-packs --in $packsOut --out $distOut --method deflate

  Write-Host "Core pack stats"
  Invoke-CargoChecked run --release -- pack-info --dir (Join-Path $packsOut "en")
} finally {
  Pop-Location
}

Write-Host "Done."
