$ErrorActionPreference = 'Stop';
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$binDir = "$($env:ChocolateyInstall)\bin"

Write-Host $toolsDir
Get-ChildItem $toolsDir

try {
    Get-Service -Name "nvml-exporter"
    Stop-Service "nvml-exporter"
    & sc.exe delete "nvml-exporter"
} catch {
    write-host "possible issue uninstalling nvml-exporter"
}
try {
    Get-Service -Name "nvml-exporter-rs"
    Stop-Service "nvml-exporter-rs"
    & sc.exe delete "nvml-exporter-rs"
} catch {
    write-host "possible issue uninstalling nvml-exporter-rs"
}

Uninstall-BinFile -Name "nvml-exporter-rs"
Install-BinFile -Path "$toolsDir\nvml_exporter_svc.exe" -Name "nvml-exporter"

New-Service -BinaryPathName "$binDir\nvml-exporter.exe" -Name "nvml-exporter" -StartupType Automatic
Start-Service "nvml-exporter"