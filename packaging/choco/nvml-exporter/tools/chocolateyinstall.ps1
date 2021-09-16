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


$arguments = @{};
# /GitOnlyOnPath /GitAndUnixToolsOnPath /NoAutoCrlf
$packageParameters = $env:chocolateyPackageParameters;

# Default the values
$enableThrottleReasons = $false
$port = 9944

write-host $packageParameters

# Now parse the packageParameters using good old regular expression
if ($packageParameters) {
    $match_pattern = "\/(?<option>([a-zA-Z]+)):(?<value>([`"'])?([a-zA-Z0-9- _\\:\.]+)([`"'])?)|\/(?<option>([a-zA-Z]+))"
    #"
    $option_name = 'option'
    $value_name = 'value'

    if ($packageParameters -match $match_pattern ){
        $results = $packageParameters | Select-String $match_pattern -AllMatches
        $results.matches | % {
          $arguments.Add(
              $_.Groups[$option_name].Value.Trim(),
              $_.Groups[$value_name].Value.Trim())
      }
    }
    else
    {
      throw "Package Parameters were found but were invalid (REGEX Failure)"
    }

    if ($arguments.ContainsKey("EnableThrottleReasons")) {
        Write-Host "You want to enable throttle reasons collection"
        $enableThrottleReasons = $true
    }
    if ($arguments.ContainsKey("ListenPort")) {
        $port = $arguments["ListenPort"]
        Write-Host "Use custom port $port"
    }
} else {
    Write-Debug "No Package Parameters Passed in";
}

Uninstall-BinFile -Name "nvml-exporter-rs"
Install-BinFile -Path "$toolsDir\nvml_exporter_svc.exe" -Name "nvml-exporter"

$binPath = "$binDir\nvml-exporter.exe"
$binPath += " --listen 0.0.0.0:$port"
$binPath += " --listen [::]:$port"
if ($enableThrottleReasons) {
    $binPath += " --throttle-reasons"
}

New-Service -BinaryPathName $binPath -Name "nvml-exporter" -StartupType Automatic
Start-Service "nvml-exporter"