echo building
cmd /c %~dp0mkchoco.bat
echo installing
cd %~dp0..\packaging\choco\nvml-exporter
choco install -y --force nvml-exporter.0.0.1.nupkg