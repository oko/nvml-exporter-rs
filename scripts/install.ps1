$installdir = 'C:\ProgramData\nvml-exporter-rs'
if (!(Test-Path -Path $installdir -PathType Container)) {
    mkdir $installdir
}
cargo build --release
cp .\target\release\nvml-exporter-rs.exe $installdir