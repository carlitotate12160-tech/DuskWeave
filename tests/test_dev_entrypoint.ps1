param()
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$entry = Join-Path $repo 'scripts/dev.ps1'
$temp = Join-Path ([IO.Path]::GetTempPath()) ('dw-dev-entry-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($temp) | Out-Null
$ps = (Get-Process -Id $PID).Path
$utf8 = New-Object Text.UTF8Encoding($false)

function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Start-Case([string]$Body, [hashtable]$Variables = @{}) {
    $path = Join-Path $temp ([guid]::NewGuid().ToString('N') + '.ps1')
    [IO.File]::WriteAllText($path, $Body, $utf8)
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $ps
    $info.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $path + '"'
    $info.WorkingDirectory = $temp
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables['PATH'] = $temp + ';' + $env:PATH
    $info.EnvironmentVariables['DW_DATABASE_URL'] = 'TEST_DSN_SENTINEL'
    $info.EnvironmentVariables['DEEPSEEK_API_KEY'] = 'TEST_PROVIDER_SENTINEL'
    $info.EnvironmentVariables['CARGO_TARGET_DIR'] = 'TEST_TARGET_SENTINEL'
    $info.EnvironmentVariables['DW_TEST_PYTHON'] = Join-Path $temp 'python.cmd'
    $info.EnvironmentVariables['DW_TEST_OPENSSL'] = Join-Path $temp 'openssl.cmd'
    $info.EnvironmentVariables['DEV_ENTRYPOINT_CAPTURE'] = Join-Path $temp 'capture.json'
    foreach ($name in $Variables.Keys) { $info.EnvironmentVariables[$name] = $Variables[$name] }
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $info
    Assert-True $process.Start() 'child did not start'
    return @{ Process = $process; Out = $process.StandardOutput.ReadToEndAsync();
        Err = $process.StandardError.ReadToEndAsync() }
}

function Finish-Case($Case) {
    if (-not $Case.Process.WaitForExit(30000)) {
        $Case.Process.Kill()
        throw 'entrypoint test exceeded its bound'
    }
    $output = $Case.Out.GetAwaiter().GetResult() + $Case.Err.GetAwaiter().GetResult()
    Assert-True ($output -notmatch 'TEST_DSN_SENTINEL|TEST_PROVIDER_SENTINEL') 'secret leaked'
    return @{ Code = $Case.Process.ExitCode; Output = $output }
}

$fakeCargo = @'
@echo off
if "%~2"=="--version" (
  echo cargo entrypoint-test
  exit /b 0
)
set "DEV_ENTRYPOINT_ARGUMENTS=%*"
if defined DEV_ENTRYPOINT_STDERR echo synthetic-progress 1>&2
if defined DEV_ENTRYPOINT_STDERR powershell.exe -NoProfile -Command "[Console]::Error.WriteLine()"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0capture.ps1"
exit /b %ERRORLEVEL%
'@
$capture = @'
@{
    arguments = @($env:DEV_ENTRYPOINT_ARGUMENTS -split ' ')
    directory = (Get-Location).Path
    target = $env:CARGO_TARGET_DIR
    coverage = $env:CARGO_LLVM_COV_TARGET_DIR
    dsn_present = [bool]$env:DW_DATABASE_URL
    provider_present = [bool]$env:DEEPSEEK_API_KEY
} | ConvertTo-Json | Set-Content -LiteralPath $env:DEV_ENTRYPOINT_CAPTURE
if ($env:DEV_ENTRYPOINT_WAIT) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not (Test-Path -LiteralPath $env:DEV_ENTRYPOINT_WAIT)) {
        if ([DateTime]::UtcNow -gt $deadline) { exit 99 }
        Start-Sleep -Milliseconds 50
    }
}
if ($env:DEV_ENTRYPOINT_EXIT) { exit ([int]$env:DEV_ENTRYPOINT_EXIT) }
exit 0
'@
[IO.File]::WriteAllText((Join-Path $temp 'cargo.cmd'), $fakeCargo, $utf8)
[IO.File]::WriteAllText((Join-Path $temp 'capture.ps1'), $capture, $utf8)
# Command-routing tests own their tool fixtures; host installations are irrelevant.
$pythonPin = [IO.File]::ReadAllText((Join-Path $repo '.python-version')).Trim()
[IO.File]::WriteAllText((Join-Path $temp 'python.cmd'), "@echo off`r`necho Python $pythonPin.0", $utf8)
[IO.File]::WriteAllText((Join-Path $temp 'openssl.cmd'), "@echo off`r`necho OpenSSL entrypoint-test", $utf8)
$escapedEntry = $entry.Replace("'", "''")
$escapedRepo = $repo.Replace("'", "''")
$success = @'
$initial = (Get-Location).Path
& 'ENTRY' build
if ($LASTEXITCODE -ne 0) { exit 71 }
if ($env:DW_DATABASE_URL -ne 'TEST_DSN_SENTINEL') { exit 72 }
if ($env:DEEPSEEK_API_KEY -ne 'TEST_PROVIDER_SENTINEL') { exit 73 }
if ($env:CARGO_TARGET_DIR -ne 'TEST_TARGET_SENTINEL') { exit 74 }
if ((Get-Location).Path -ne $initial) { exit 75 }
exit 0
'@
$success = $success.Replace('ENTRY', $escapedEntry)
try {
    $result = Finish-Case (Start-Case $success)
    Assert-True ($result.Code -eq 0) 'successful command failed or changed parent environment'
    $observed = Get-Content -Raw (Join-Path $temp 'capture.json') | ConvertFrom-Json
    Assert-True (-not $observed.dsn_present -and -not $observed.provider_present) 'child received inherited credentials'
    Assert-True ($observed.directory -eq $repo) 'non-root invocation used wrong working directory'
    Assert-True ($observed.target.EndsWith('normal')) 'ordinary target was not selected'
    Assert-True (-not $observed.coverage) 'ordinary command inherited coverage settings'
    Write-Output 'PASS child isolation, parent restoration, non-root cwd and ordinary target'

    $redirected = $success.Replace("' build", "' build 2>&1 | Out-String | Write-Output")
    $result = Finish-Case (Start-Case $redirected @{DEV_ENTRYPOINT_STDERR='1'})
    Assert-True ($result.Code -eq 0 -and $result.Output.Contains('synthetic-progress')) 'native stderr was lost or treated as failure'
    Assert-True ($result.Output -notmatch 'NativeCommandError|System.Management.Automation.RemoteException') 'native progress became a PowerShell error record'
    Write-Output 'PASS native stderr forwarding without false errors'

    $untracked = "& '" + $escapedEntry + "' test -Target dev_untracked_probe; exit " + '$LASTEXITCODE'
    $result = Finish-Case (Start-Case $untracked)
    Assert-True ($result.Code -ne 0 -and $result.Output.Contains('dev_target_untracked')) 'untracked target reached a child command'
    Write-Output 'PASS untracked integration target refusal'

    $result = Finish-Case (Start-Case ("& '" + $escapedEntry + "' test; exit " + '$LASTEXITCODE'))
    Assert-True ($result.Code -eq 0) ('default unit command failed: ' + $result.Output)
    $observed = Get-Content -Raw (Join-Path $temp 'capture.json') | ConvertFrom-Json
    Assert-True ($observed.arguments -contains '--lib' -and $observed.arguments -notcontains '--bin') 'default unit command included database-dependent binary tests'
    Write-Output 'PASS database-free library selection'

    $failure = @'
& 'ENTRY' build
$code = $LASTEXITCODE
if ($env:DW_DATABASE_URL -ne 'TEST_DSN_SENTINEL') { exit 76 }
exit $code
'@
    $result = Finish-Case (Start-Case ($failure.Replace('ENTRY', $escapedEntry)) @{DEV_ENTRYPOINT_EXIT='42'})
    Assert-True ($result.Code -eq 42) 'child exit code was not preserved'
    Write-Output 'PASS failure exit propagation and restoration'

    [IO.File]::WriteAllText((Join-Path $temp 'python.cmd'), "@echo off" + [Environment]::NewLine + "echo Python 3.11.0", $utf8)
    $result = Finish-Case (Start-Case ("& '" + $escapedEntry + "' check; exit " + '$LASTEXITCODE'))
    Assert-True ($result.Code -ne 0 -and $result.Output.Contains('dev_python_version_refused')) 'unsupported Python was accepted'
    Write-Output 'PASS Python version refusal'

    $release = Join-Path $temp 'release'
    Remove-Item -LiteralPath (Join-Path $temp 'capture.json')
    $first = Start-Case $success @{DEV_ENTRYPOINT_WAIT=$release}
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while (-not (Test-Path (Join-Path $temp 'capture.json'))) {
            Assert-True ([DateTime]::UtcNow -lt $deadline) 'first owner did not enter'
            Start-Sleep -Milliseconds 50
        }
        $second = Finish-Case (Start-Case $success)
        Assert-True ($second.Code -ne 0 -and $second.Output.Contains('dev_resources_busy')) 'concurrent resource owner was admitted'
    }
    finally { [IO.File]::WriteAllText($release, '', $utf8) }
    $result = Finish-Case $first
    Assert-True ($result.Code -eq 0) 'first resource owner failed'
    Write-Output 'PASS concurrent entrypoint exclusion'

    $result = Finish-Case (Start-Case $success)
    Assert-True ($result.Code -eq 0) 'resource lock was not released'
    Write-Output 'PASS resource release after completion'

    $fakeDocker = @'
@echo off
if "%~1"=="start" (
  echo touched>"%DEV_ENTRYPOINT_DOCKER_START%"
  exit /b 0
)
if "%~3"=="{{.Config.Image}}" (
  echo %DEV_ENTRYPOINT_IMAGE%
  exit /b 0
)
if "%~3"=="{{json .HostConfig.PortBindings}}" (
  if defined DEV_ENTRYPOINT_SAFE_BINDING (
    echo {"5432/tcp":[{"HostIp":"127.0.0.1","HostPort":"5432"}]}
  ) else (
    echo {"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"5432"}]}
  )
  exit /b 0
)
if "%~3"=="{{.State.Status}}" (
  echo running
  exit /b 0
)
if "%~1"=="exec" exit /b 0
exit /b 98
'@
    [IO.File]::WriteAllText((Join-Path $temp 'docker.cmd'), $fakeDocker, $utf8)
    $marker = Join-Path $temp 'docker-started'
    $postgres = "& '" + $escapedEntry + "' postgres; exit " + '$LASTEXITCODE'
    $result = Finish-Case (Start-Case $postgres @{
        DEV_ENTRYPOINT_IMAGE='postgres:wrong'
        DEV_ENTRYPOINT_DOCKER_START=$marker
    })
    Assert-True ($result.Code -ne 0 -and $result.Output.Contains('dev_postgres_image_refused')) 'unqualified image was admitted'
    Assert-True (-not (Test-Path $marker)) 'unqualified container was started'
    Write-Output 'PASS PostgreSQL image refusal before start'

    $result = Finish-Case (Start-Case $postgres @{
        DEV_ENTRYPOINT_IMAGE='postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f'
        DEV_ENTRYPOINT_DOCKER_START=$marker
    })
    Assert-True ($result.Code -ne 0 -and $result.Output.Contains('dev_postgres_binding_refused')) 'unsafe port binding was admitted'
    Assert-True (-not (Test-Path $marker)) 'unsafe container was started'
    Write-Output 'PASS PostgreSQL binding refusal before start'

    $missing = "& '" + $escapedEntry + "' postgres -ArtifactRoot '" + (Join-Path $temp 'missing-helper') + "'; exit " + '$LASTEXITCODE'
    $result = Finish-Case (Start-Case $missing @{
        DEV_ENTRYPOINT_IMAGE='postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f'
        DEV_ENTRYPOINT_SAFE_BINDING='1'
    })
    Assert-True ($result.Code -ne 0 -and $result.Output.Contains('dev_command_start_failed')) 'missing executable was reported as success'
    Write-Output 'PASS missing executable cannot qualify a service'

}
finally {
    Remove-Item -LiteralPath $temp -Recurse -Force
}
