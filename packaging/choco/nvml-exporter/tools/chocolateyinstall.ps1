$ErrorActionPreference = 'Stop';
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$arguments = @{};
# /ListenPort: /EnableThrottleReasons
$packageParameters = $env:chocolateyPackageParameters;

# Default parameter values
## /EnableThrottleReasons
$enableThrottleReasons = $false
## /ListenPort:
$port = 9944

if ($packageParameters) {
    $match_pattern = "\/(?<option>([a-zA-Z]+)):(?<value>([`"'])?([a-zA-Z0-9- _\\:\.]+)([`"'])?)|\/(?<option>([a-zA-Z]+))"
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
        Write-Host "Param: enable throttle reasons collections"
        $enableThrottleReasons = $true
    }
    if ($arguments.ContainsKey("ListenPort")) {
        $port = $arguments["ListenPort"]
        Write-Host "Param: use custom port $port"
    }
} else {
    Write-Debug "No Package Parameters Passed in";
}

$binPath = "$toolsDir\nvml_exporter_svc.exe"
# TODO(https://github.com/oko/nvml-exporter-rs/issues/2): allow listen address configuration
$binPath += " --listen 0.0.0.0:$port"
$binPath += " --listen [::]:$port"
if ($enableThrottleReasons) {
    $binPath += " --throttle-reasons"
}

try {
    # get service to check if this is an upgrade
    Get-Service -Name "nvml-exporter"
} catch {
    # create service if it doesn't exist
    New-Service -BinaryPathName $binPath -Name "nvml-exporter" -StartupType Automatic
}
Start-Service "nvml-exporter"