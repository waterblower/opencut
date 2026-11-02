$ErrorActionPreference = 'Stop'
$rustRoot = Split-Path -Parent $PSScriptRoot
if (-not $env:FFMPEG_DIR) { $env:FFMPEG_DIR = Join-Path $rustRoot 'vendor/ffmpeg-8.1.2' }
$env:Path = "$env:FFMPEG_DIR\bin;$env:Path"
Set-Location -LiteralPath $rustRoot
if ($args.Count -gt 0 -and $args[0] -eq '--exec') {
    $executable = $args[1]
    # @() keeps a single argument an array; splatting a bare string passes each character.
    $programArguments = @($args | Select-Object -Skip 2)
    & $executable @programArguments
    exit $LASTEXITCODE
}
& cargo @args
exit $LASTEXITCODE
