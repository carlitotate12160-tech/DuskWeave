[CmdletBinding()]
param(
    [ValidateSet('build', 'check', 'test', 'coverage', 'postgres')]
    [string]$Action = 'check',
    [string]$Target,
    [switch]$Postgres,
    [string]$ArtifactRoot,
    [string]$Python = $env:DW_TEST_PYTHON,
    [string]$OpenSsl = $env:DW_TEST_OPENSSL
)
$ErrorActionPreference = 'Stop'
$script:ExitCode = 1
$repo = Split-Path $PSScriptRoot -Parent

function Find-Executable([string]$Name) {
    $tool = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $tool) { throw 'dev_tool_missing' }
    return $tool.Source
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    # Windows PowerShell 5.1 may represent native stderr as ErrorRecord objects.
    $ErrorActionPreference = 'Continue'
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        $script:ExitCode = $LASTEXITCODE
        throw 'dev_command_failed'
    }
}

function Read-Checked([string]$Program, [string[]]$Arguments) {
    return (Invoke-Checked $Program $Arguments | Out-String).Trim()
}

function Select-Python([string]$Requested) {
    if (-not $Requested) { $Requested = 'python' }
    $tool = Find-Executable $Requested
    $pin = [IO.File]::ReadAllText((Join-Path $repo '.python-version')).Trim()
    $version = Read-Checked $tool @('--version')
    if ($version -notmatch ('^Python ' + [regex]::Escape($pin) + '\.\d+$')) {
        throw 'dev_python_version_refused'
    }
    Write-Host "python=$version"
    return $tool
}

function Select-OpenSsl([string]$Requested) {
    if (-not $Requested) {
        $found = Get-Command openssl -CommandType Application -ErrorAction SilentlyContinue
        if ($found) { $Requested = $found.Source }
        else { $Requested = Join-Path $env:ProgramFiles 'Git/mingw64/bin/openssl.exe' }
    }
    $tool = Find-Executable $Requested
    Write-Host ('openssl=' + (Read-Checked $tool @('version')))
    return $tool
}

function Wait-Postgres([string]$Docker) {
    $ErrorActionPreference = 'Continue'
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        & $Docker exec dwpg-r3 pg_isready -U postgres *> $null
        if ($LASTEXITCODE -eq 0) { return }
        Start-Sleep -Milliseconds 200
    }
    throw 'dev_postgres_not_ready'
}

function Use-Postgres([string]$Supervisor) {
    $docker = Find-Executable 'docker'
    $image = Read-Checked $docker @('inspect', '--format', '{{.Config.Image}}', 'dwpg-r3')
    $pin = 'postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f'
    if ($image -ne $pin) { throw 'dev_postgres_image_refused' }
    $ports = (Read-Checked $docker @('inspect', '--format',
        '{{json .HostConfig.PortBindings}}', 'dwpg-r3')) | ConvertFrom-Json
    $binding = @($ports.'5432/tcp')
    if (@($ports.PSObject.Properties).Count -ne 1 -or $binding.Count -ne 1 -or
        $binding[0].HostIp -ne '127.0.0.1' -or $binding[0].HostPort -ne '5432') {
        throw 'dev_postgres_binding_refused'
    }
    $state = Read-Checked $docker @('inspect', '--format', '{{.State.Status}}', 'dwpg-r3')
    if ($state -ne 'running') { Invoke-Checked $docker @('start', 'dwpg-r3') }
    Wait-Postgres $docker
    Invoke-Checked $Supervisor @('service-check', '--container', 'dwpg-r3',
        '--version-exact', '17.11')
}

function Run-Tests([bool]$Coverage) {
    $arguments = @("+$toolchain")
    if ($Coverage) { $arguments += 'llvm-cov' } else { $arguments += 'test' }
    $arguments += @('--locked', '--features', 'fixture-supervisor')
    if ($Target) { $arguments += @('--test', $Target) }
    else { $arguments += @('--lib', '--bin', 'duskweave') }
    if ($Coverage) {
        $arguments += @('--json', '--output-path', (Join-Path $report 'coverage.json'))
    }
    $arguments += @('--', '--test-threads=1')
    if ($Postgres) {
        $bootstrap = Join-Path $repo 'tests/support/m1_native_bootstrap.py'
        Invoke-Checked $pythonTool (@($bootstrap, '--container', 'dwpg-r3',
            '--report', $report, '--', $cargo) + $arguments)
    } else { Invoke-Checked $cargo $arguments }
}

$mutex = $null
$ownsMutex = $false
$savedEnvironment = $null
$managedNames = @()
Push-Location $repo
try {
    if ($env:OS -ne 'Windows_NT') { throw 'dev_windows_required' }
    if ($Target -and $Target -notmatch '^[a-z][a-z0-9_]*$') { throw 'dev_target_refused' }
    if ($Postgres -and (-not $Target -or $Action -notin @('test', 'coverage'))) {
        throw 'dev_postgres_requires_test_target'
    }
    if ($Target -and $Action -notin @('test', 'coverage')) { throw 'dev_target_refused' }
    # One same-user Windows session owns build targets and cluster-role fixtures.
    $mutex = New-Object Threading.Mutex($false, 'Local\DuskWeave.DeveloperVerification')
    try { $ownsMutex = $mutex.WaitOne(0) }
    catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
    if (-not $ownsMutex) { throw 'dev_resources_busy' }

    $cargo = Find-Executable 'cargo'
    $git = Find-Executable 'git'
    if ($Target -and -not (Read-Checked $git @('ls-files', '--', "tests/$Target.rs"))) {
        throw 'dev_target_untracked'
    }
    $toolchain = [regex]::Match(
        [IO.File]::ReadAllText((Join-Path $repo 'rust-toolchain.toml')),
        '(?m)^channel\s*=\s*"(\d+\.\d+\.\d+)"').Groups[1].Value
    if (-not $toolchain) { throw 'dev_toolchain_unpinned' }
    $common = Read-Checked $git @('rev-parse', '--path-format=absolute', '--git-common-dir')
    if (-not $ArtifactRoot) {
        $family = Split-Path (Split-Path $common -Parent) -Parent
        $ArtifactRoot = Join-Path $family "DuskWeave-build/rust-$toolchain"
    }
    $ArtifactRoot = [IO.Path]::GetFullPath($ArtifactRoot)
    $normal = Join-Path $ArtifactRoot 'normal'
    $coverageTarget = Join-Path $ArtifactRoot 'coverage/dev'
    $run = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ') + '-' + $PID
    $report = Join-Path $ArtifactRoot "reports/dev/$run"
    [IO.Directory]::CreateDirectory($normal) | Out-Null

    $savedEnvironment = [Environment]::GetEnvironmentVariables('Process')
    $managedNames = @($savedEnvironment.Keys | Where-Object { $_ -like 'DW_*' }) +
        @('DEEPSEEK_API_KEY', 'CARGO_TARGET_DIR', 'CARGO_LLVM_COV_TARGET_DIR',
          'CARGO_BUILD_TARGET', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
          'LLVM_PROFILE_FILE', 'PYTHONDONTWRITEBYTECODE',
          'DW_TEST_PYTHON', 'DW_TEST_OPENSSL', 'DW_TEST_FIXTURE_SUPERVISOR')
    $managedNames = @($managedNames | Select-Object -Unique)
    foreach ($name in $managedNames) {
        [Environment]::SetEnvironmentVariable($name, $null, 'Process')
    }
    $env:CARGO_TARGET_DIR = $normal
    $env:PYTHONDONTWRITEBYTECODE = '1'
    Write-Host ('git_head=' + (Read-Checked $git @('rev-parse', 'HEAD')))
    Write-Host 'verification_source=current_worktree'
    Write-Host "normal_target=$normal"
    Write-Host ('cargo=' + (Read-Checked $cargo @("+$toolchain", '--version')))

    if ($Action -in @('check', 'test', 'coverage')) {
        $pythonTool = Select-Python $Python
        $env:DW_TEST_PYTHON = $pythonTool
    }
    if ($Action -in @('test', 'coverage')) {
        $env:DW_TEST_OPENSSL = Select-OpenSsl $OpenSsl
    }
    if ($Action -eq 'coverage') {
        $version = Read-Checked $cargo @("+$toolchain", 'llvm-cov', '--version')
        if ($version -ne 'cargo-llvm-cov 0.9.1') { throw 'dev_coverage_version_refused' }
        $env:CARGO_LLVM_COV_TARGET_DIR = $coverageTarget
        [IO.Directory]::CreateDirectory($report) | Out-Null
        Write-Host "coverage_target=$coverageTarget"
        Write-Host "report=$report"
    }
    if ($Postgres -or $Action -eq 'postgres') {
        Invoke-Checked $cargo @("+$toolchain", 'build', '--locked',
            '--features', 'fixture-supervisor', '--bin', 'dw-fixture-supervisor')
        $supervisor = Join-Path $normal 'debug/dw-fixture-supervisor.exe'
        $env:DW_TEST_FIXTURE_SUPERVISOR = $supervisor
        Use-Postgres $supervisor
        Write-Host "report=$report"
    }

    switch ($Action) {
        'build' { Invoke-Checked $cargo @("+$toolchain", 'build', '--locked') }
        'check' {
            Invoke-Checked $cargo @("+$toolchain", 'fmt', '--', '--check')
            Invoke-Checked $cargo @("+$toolchain", 'clippy', '--locked', '--all-targets',
                '--features', 'fixture-supervisor', '--', '-D', 'warnings')
            foreach ($checker in @('check_links.py', 'check_structure.py')) {
                Invoke-Checked $pythonTool @('-B', (Join-Path $repo "scripts/$checker"))
            }
        }
        'test' { Run-Tests $false }
        'coverage' {
            Invoke-Checked $cargo @("+$toolchain", 'llvm-cov', 'clean', '--workspace')
            Run-Tests $true
            Invoke-Checked $cargo @("+$toolchain", 'llvm-cov', 'report', '--text',
                '--output-path', (Join-Path $report 'coverage.txt'))
            Write-Host 'Scoped coverage only; the full-suite 90/80 gate remains in CI.'
        }
        'postgres' { Write-Host 'Owned PostgreSQL qualified; no database or login was created.' }
    }
    $script:ExitCode = 0
}
catch {
    $category = $_.Exception.Message
    if ($category -notmatch '^dev_[a-z_]+$') { $category = 'dev_preflight_failed' }
    [Console]::Error.WriteLine("$category (exit=$script:ExitCode)")
}
finally {
    foreach ($name in $managedNames) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
    }
    if ($ownsMutex) { $mutex.ReleaseMutex() }
    if ($mutex) { $mutex.Dispose() }
    Pop-Location
}
exit $script:ExitCode
