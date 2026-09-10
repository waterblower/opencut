$ErrorActionPreference = 'Stop'
$rustRoot = Split-Path -Parent $PSScriptRoot
$gstRoot = Join-Path $rustRoot 'vendor/gstreamer'
if (-not $env:FFMPEG_DIR) { $env:FFMPEG_DIR = Join-Path $rustRoot 'vendor/ffmpeg-8.1.2' }
$env:Path = "$gstRoot\bin;$env:FFMPEG_DIR\bin;$env:Path"
$env:PKG_CONFIG = "$gstRoot\bin\pkg-config.exe"
$env:PKG_CONFIG_PATH = "$gstRoot\lib\pkgconfig"
$env:GST_PLUGIN_PATH_1_0 = ''
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = "$gstRoot\lib\gstreamer-1.0"
$env:GST_PLUGIN_SCANNER_1_0 = "$gstRoot\libexec\gstreamer-1.0\gst-plugin-scanner.exe"
$env:GST_REGISTRY_1_0 = "$gstRoot\registry-windows.bin"
$env:GIO_EXTRA_MODULES = "$gstRoot\lib\gio\modules"
# Hardware H.264 decoding corrupts frames on the Intel Iris Xe setup.
if (-not $env:GST_PLUGIN_FEATURE_RANK) {
    $env:GST_PLUGIN_FEATURE_RANK = 'avdec_h264:MAX,d3d11h264dec:NONE,d3d12h264dec:NONE,qsvh264dec:NONE'
}
Set-Location -LiteralPath $rustRoot
if ($args.Count -gt 0 -and $args[0] -eq '--exec') {
    $executable, $programArguments = $args[1..($args.Count - 1)]
    & $executable @programArguments
    exit $LASTEXITCODE
}
& cargo @args
exit $LASTEXITCODE
