$ErrorActionPreference = 'Stop'; # stop on all errors
Start-ChocolateyProcessAsAdmin -ExeToRun "sc.exe" -Statements "delete","nvml-exporter"