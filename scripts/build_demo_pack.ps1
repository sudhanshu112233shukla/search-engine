$ErrorActionPreference = "Stop"

# Builds a demo pack suitable for sharing:
# - Creates a smaller dataset JSONL (sampling from existing imports)
# - Builds shard indexes
# - Exports shard zips for Android downloader
#
# Assumptions:
# - You already imported data into search_engine_rust\data\*.jsonl
# - You already created wiki_200k.jsonl and warc_50k.jsonl (or have wiki.jsonl/warc.jsonl)
#
# Output:
# - search_engine_rust\data\packs_demo\  (built shard indexes)
# - search_engine_rust\dist\packs_demo\  (manifest.json + shard_XXXX.zip)

$root = Split-Path -Parent $PSScriptRoot
$rust = Join-Path $root "search_engine_rust"

$data = Join-Path $rust "data"
$indexDir = Join-Path $data "index"
$datasetOut = Join-Path $indexDir "dataset_demo.jsonl"

New-Item -ItemType Directory -Force -Path $indexDir | Out-Null

function Sample-Jsonl([string]$inPath, [string]$outPath, [int]$n) {
  if (!(Test-Path $inPath)) { throw "Missing file: $inPath" }
  Write-Host "Sample $n lines: $inPath -> $outPath"
  Get-Content $inPath -TotalCount $n | Set-Content $outPath
}

# Choose a demo-sized corpus. Adjust these to hit ~3-5GB pack sizes.
$wikiIn = Join-Path $data "wiki_200k.jsonl"
if (!(Test-Path $wikiIn)) { $wikiIn = Join-Path $data "wiki.jsonl" }
$warcIn = Join-Path $data "warc_50k.jsonl"
if (!(Test-Path $warcIn)) { $warcIn = Join-Path $data "warc.jsonl" }

$wikiSample = Join-Path $data "wiki_demo_100k.jsonl"
$warcSample = Join-Path $data "warc_demo_20k.jsonl"

Sample-Jsonl $wikiIn $wikiSample 100000
Sample-Jsonl $warcIn $warcSample 20000

$osmFiles = @(
  @{ in = (Join-Path $data "osm_india.jsonl"); out = (Join-Path $data "osm_india_demo_15000.jsonl"); n = 15000 },
  @{ in = (Join-Path $data "osm_korea.jsonl"); out = (Join-Path $data "osm_korea_demo_8000.jsonl"); n = 8000 },
  @{ in = (Join-Path $data "osm_camerica.jsonl"); out = (Join-Path $data "osm_camerica_demo_4000.jsonl"); n = 4000 },
  @{ in = (Join-Path $data "osm_samerica.jsonl"); out = (Join-Path $data "osm_samerica_demo_4000.jsonl"); n = 4000 },
  @{ in = (Join-Path $data "osm_aus.jsonl"); out = (Join-Path $data "osm_aus_demo_4000.jsonl"); n = 4000 }
)

foreach ($o in $osmFiles) {
  if (Test-Path $o.in) { Sample-Jsonl $o.in $o.out $o.n }
}

# Merge into a single JSONL file (fast, no huge intermediate JSON array).
Write-Host "Merging -> $datasetOut"
if (Test-Path $datasetOut) { Remove-Item $datasetOut -Force }

$mergeList = @(
  $wikiSample
  $osmFiles | ForEach-Object { $_.out } | Where-Object { Test-Path $_ }
  $warcSample
) | ForEach-Object { $_ }  # flatten

$cmd = "copy /b " + (($mergeList | ForEach-Object { '"' + $_ + '"' }) -join "+") + " " + ('"' + $datasetOut + '"')
cmd /c $cmd | Out-Null

# Build packs with smaller shard size to keep each shard zip under typical hosting limits.
$packsOut = Join-Path $data "packs_demo"

Push-Location $rust
try {
  Write-Host "Building packs -> $packsOut"
  cargo run --release -- pack --dataset "data/index/dataset_demo.jsonl" --out "data/packs_demo" --max-docs 40000 --lang en

  Write-Host "Validating packs"
  cargo run --release -- validate-pack --dir "data/packs_demo/en" --smoke-query "what is google"

  Write-Host "Exporting zip shards -> dist/packs_demo"
  cargo run --release -- export-packs --in "data/packs_demo" --out "dist/packs_demo" --method stored
} finally {
  Pop-Location
}

Write-Host "Done."

