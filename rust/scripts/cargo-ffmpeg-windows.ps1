$ErrorActionPreference = 'Stop'
$rustRoot = Split-Path -Parent $PSScriptRoot
if (-not $env:FFMPEG_DIR) { $env:FFMPEG_DIR = Join-Path $rustRoot 'vendor/ffmpeg-8.1.2' }
$env:Path = "$env:FFMPEG_DIR\bin;$env:Path"
Set-Location -LiteralPath $rustRoot
if ($args.Count -gt 0 -and $args[0] -eq '--exec') {
    $executable, $programArguments = $args[1..($args.Count - 1)]
    & $executable @programArguments
    exit $LASTEXITCODE
}
& cargo @args
exit $LASTEXITCODE
