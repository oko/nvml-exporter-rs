echo building
cmd /c %~dp0mkchoco.bat
echo installing
cd %~dp0..\packaging\choco\nvml-exporter
powershell.exe -executionpolicy bypass %~dp0install.ps1