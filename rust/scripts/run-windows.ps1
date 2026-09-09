$ErrorActionPreference = 'Stop'
$rustRoot = Split-Path -Parent $PSScriptRoot
$env:Path = "$rustRoot\vendor\gstreamer\bin;$rustRoot\vendor\ffmpeg-8.1.2\bin;$env:Path"
$program = $args[0]
$programArguments = @($args | Select-Object -Skip 1)
& $program @programArguments
exit $LASTEXITCODE
