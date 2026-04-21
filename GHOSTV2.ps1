<#
.SYNOPSIS
    AMD Ghost Environment (Consumer Edition v2.7 - Final Master)
.DESCRIPTION
    A native PowerShell daemon that spoofs up to two AMD RDNA/Vega GPUs as NVIDIA GPUs,
    injects the ZLUDA translation layer, and provides a Waiting Room TUI with DOOM.
#>

$ErrorActionPreference = "Continue";

# SUB-PROCESS ENCODING LOSS (Force UTF-8 at the console level)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
[Console]::InputEncoding = [System.Text.Encoding]::UTF8;

$GhostDir = Join-Path $env:USERPROFILE ".ghost";
$ZludaDir = Join-Path $GhostDir "zluda";
$DoomDir = Join-Path $GhostDir "doom";
$MusicDir = Join-Path $GhostDir "music";
$ConfigDir = Join-Path $env:USERPROFILE ".config";
$ConfigFile = Join-Path $ConfigDir "ghost_env_setup.ps1";

# ANTI-NESTING LOCK
if ($env:GHOST_ENV_ACTIVE -eq "1" -and $MyInvocation.Line -notmatch "ghost-start") {
    Write-Host "[GHOST] You are already inside the Ghost Environment." -ForegroundColor Green;
    exit;
}

# ==============================================================================
# PHASE 1: DUAL-GPU DETECTION & JSON MAPPING
# ==============================================================================

function Get-AmdGpuCluster {
    $gpus = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "AMD|Radeon|Advanced Micro Devices|Instinct" });
    
    $gpuList = @();
    $rawIndex = 0; # DYNAMIC INDEXING: Track raw OS index for HIP/ROCR
    
    foreach ($gpu in $gpus) {
        $gpuObj = [PSCustomObject]@{
            Name = $gpu.Name
            AdapterRAM = $gpu.AdapterRAM
            PNPDeviceID = $gpu.PNPDeviceID
            OsIndex = $rawIndex
        };
        
        # FIX 4: REGISTRY WILDCARD "RACE" (Match strictly by unique PNPDeviceID)
        $pnpDevice = Get-CimInstance Win32_PnPEntity -Filter "DeviceID='$($gpu.PNPDeviceID -replace '\\', '\\')'" -ErrorAction SilentlyContinue;
        if ($pnpDevice) {
            $driverKey = $pnpDevice.GetCimSession().QueryInstances("root\cimv2", "WQL", "SELECT * FROM Win32_PnPSignedDriver WHERE DeviceID='$($pnpDevice.DeviceID -replace '\\', '\\')'") | Select-Object -ExpandProperty DriverName -ErrorAction SilentlyContinue;
            
            if ($driverKey) {
                $exactRegPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\$driverKey";
                $regData = Get-ItemProperty -Path $exactRegPath -Name "HardwareInformation.qwMemorySize" -ErrorAction SilentlyContinue;
                
                if ($regData -and $null -ne $regData."HardwareInformation.qwMemorySize") {
                    $gpuObj.AdapterRAM = $regData."HardwareInformation.qwMemorySize";
                }
            }
        }
        
        # iGPU "IMPOSTER" SYNDROME (Filter out GPUs with < 2GB VRAM)
        if ($gpuObj.AdapterRAM -ge 2147483648) {
            $gpuList += $gpuObj;
        }
        $rawIndex++;
    }
    
    return ($gpuList | Sort-Object -Property AdapterRAM -Descending | Select-Object -First 2);
}

function Set-GhostEnvironment {
    $cluster = Get-AmdGpuCluster;
    $primaryGpuName = if ($cluster.Count -gt 0) { $cluster[0].Name } else { "UNKNOWN" };
    $series = "UNKNOWN";

    # GFX9 ISA ARCHITECTURE SPLIT (Strict separation of Vega 10 and Vega 20)
    if ($primaryGpuName -match "(?:RX|Pro W)\s*(9|8|7|6|5)\d{3}\b") {
        $series = ($matches[1] + "000");
    } elseif ($primaryGpuName -match "Radeon\s*(8|7|6)\d{2}M\b") {
        $series = ($matches[1] + "000");
    } elseif ($primaryGpuName -match "MI50|Radeon\s*VII") {
        $series = "MI50";
    } elseif ($primaryGpuName -match "Vega\s*(56|64)") {
        $series = "VEGA" + $matches[1];
    } elseif ($primaryGpuName -match "Vega\s*Frontier") {
        $series = "VEGA64";
    }
    
    $scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Get-Location };
    $parentDir = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($scriptDir, ".."));
    
    $searchPaths = @(
        Join-Path $scriptDir "configs\mapping.json",
        Join-Path $parentDir "configs\mapping.json",
        Join-Path (Get-Location) "configs\mapping.json",
        Join-Path $scriptDir "config\mapping.json",
        Join-Path $parentDir "config\mapping.json",
        Join-Path $scriptDir "mapping.json",
        Join-Path $parentDir "mapping.json"
    );

    $mappingFile = "";
    foreach ($path in $searchPaths) {
        if (Test-Path $path) {
            $mappingFile = $path;
            break;
        }
    }

    $gfx_mask = "11.0.0";
    $spoof_name = "Unknown AMD GPU (Spoofed)";

    if ($mappingFile -ne "") {
        try {
            $json = Get-Content $mappingFile -Raw | ConvertFrom-Json;
            if ($series -ne "UNKNOWN" -and $json."$series") {
                $gfx_mask = $json."$series".mask;
                $spoof_name = $json."$series".spoof;
            } elseif ($json.DEFAULT) {
                $gfx_mask = $json.DEFAULT.mask;
                $spoof_name = $json.DEFAULT.spoof;
            }
        } catch { Write-Host "[GHOST] Warning: Failed to parse mapping.json. Using safe defaults." -ForegroundColor Yellow; }
    }

    # DYNAMIC INDEXING: Use raw OS indices for HIP/ROCR
    $hipArray = @();
    foreach ($g in $cluster) { $hipArray += $g.OsIndex; }
    $hipDevices = if ($hipArray.Count -gt 0) { $hipArray -join "," } else { "0" };
    
    if ($cluster.Count -gt 1) {
        $spoof_name += " (Dual-GPU)";
    }

    [Environment]::SetEnvironmentVariable("HSA_OVERRIDE_GFX_VERSION", $gfx_mask, "Process");
    [Environment]::SetEnvironmentVariable("HIP_VISIBLE_DEVICES", $hipDevices, "Process");
    
    # ROCR_VISIBLE_DEVICES OMISSION (Required for ROCm 6.x+ on Windows)
    [Environment]::SetEnvironmentVariable("ROCR_VISIBLE_DEVICES", $hipDevices, "Process");
    
    [Environment]::SetEnvironmentVariable("CUDA_VERSION", "12.4", "Process");
    [Environment]::SetEnvironmentVariable("NVIDIA_VISIBLE_DEVICES", "all", "Process");
    [Environment]::SetEnvironmentVariable("GHOST_SPOOF_NAME", $spoof_name, "Process");
    [Environment]::SetEnvironmentVariable("GHOST_ENV_ACTIVE", "1", "Process");
    
    $seriesNum = 7000;
    if ($series -match "^\d+$") { $seriesNum = [int]$series; }
    
    # SDMA BLANKET TOGGLE (Only disable for 9000 series, enable for 7000/6000)
    if ($seriesNum -ge 9000) {
        [Environment]::SetEnvironmentVariable("HSA_ENABLE_SDMA", "0", "Process");
    } else {
        [Environment]::SetEnvironmentVariable("HSA_ENABLE_SDMA", "1", "Process");
    }
    
    $cleanZluda = $ZludaDir.TrimEnd('\');
    if ($env:PATH -notmatch [regex]::Escape($cleanZluda)) {
        $env:PATH = "$cleanZluda;" + $env:PATH;
    }

    Write-Host "[GHOST] Environment Active. $($cluster.Count) GPU(s) Spoofed as $spoof_name. GFX Masked to $gfx_mask." -ForegroundColor Green;
}

# ==============================================================================
# PHASE 2: ZLUDA AUTO-INSTALLER & WIZARD
# ==============================================================================

function Install-Zluda {
    Write-Host "[GHOST] Downloading Windows ZLUDA Engine..." -ForegroundColor Cyan;
    if (Test-Path $ZludaDir) { Remove-Item -Recurse -Force $ZludaDir; }
    New-Item -ItemType Directory -Force -Path $ZludaDir | Out-Null;

    $zipPath = "$env:TEMP\zluda_latest_$PID.zip";
    
    $mirrors = @(
        "https://github.com/lshqqytiger/ZLUDA/releases/download/v3.0.0/ZLUDA-windows-amd64.zip",
        "https://github.com/vosen/zluda/releases/download/v3/zluda-3-windows-amd64.zip"
    );

    $downloaded = $false;
    foreach ($url in $mirrors) {
        try {
            Invoke-WebRequest -Uri $url -OutFile $zipPath -ErrorAction Stop;
            $downloaded = $true;
            break;
        } catch { continue; }
    }

    if (-not $downloaded) {
        Write-Host "[GHOST] CRITICAL: All ZLUDA mirrors failed. Check your internet connection." -ForegroundColor Red;
        return;
    }
    
    Write-Host "[GHOST] Extracting ZLUDA..." -ForegroundColor Cyan;
    Expand-Archive -Path $zipPath -DestinationPath $ZludaDir -Force;
    Remove-Item $zipPath -ErrorAction SilentlyContinue;

    $extractedExe = Get-ChildItem -Path $ZludaDir -Filter "zluda.exe" -Recurse | Select-Object -First 1;
    if ($extractedExe -and $extractedExe.DirectoryName -ne $ZludaDir) {
        Move-Item -Path "$($extractedExe.DirectoryName)\*" -Destination $ZludaDir -Force;
        Remove-Item -Recurse -Force $extractedExe.DirectoryName;
    }

    if (Test-Path "$ZludaDir\zluda.exe") {
        Write-Host "[GHOST] ZLUDA installed successfully." -ForegroundColor Green;
    } else {
        Write-Host "[GHOST] CRITICAL: ZLUDA extraction failed." -ForegroundColor Red;
    }
}

function Run-Wizard {
    if (-not (Test-Path $ConfigDir)) { New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null; }
    
    Clear-Host;
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan;
    Write-Host "║                 GHOST FIRST-STARTUP WIZARD               ║" -ForegroundColor Cyan;
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan;
    Write-Host "Welcome to Ghost. What tool are you initializing?";
    Write-Host " (1) SwarmUI";
    Write-Host " (2) SD.Next";
    Write-Host " (3) Forge/A1111";
    Write-Host " (4) vLLM/Custom";
    $choice = Read-Host "Select an option [1-4]";

    $targetPort = 7860;
    $patchLogic = "";

    switch ($choice) {
        "1" { $targetPort = 7801; $patchLogic = "`$env:TORCH_ROCM_AOTRITON_ENABLE_EXPERIMENTAL='1'"; }
        "2" { $targetPort = 7860; $patchLogic = "`$env:HSA_ENABLE_SDMA='0'"; }
        "3" { $targetPort = 7860; $patchLogic = "`$env:COMMANDLINE_ARGS='--opt-sdp-attention'"; }
        default { $targetPort = 8000; $patchLogic = "`$env:GPU_MEMORY_UTILIZATION='0.90'"; }
    }

    "`$env:GHOST_TARGET_PORT='$targetPort'" | Out-File -FilePath $ConfigFile -Encoding utf8;
    $patchLogic | Out-File -FilePath $ConfigFile -Append -Encoding utf8;

    if (-not (Test-Path "$ZludaDir\zluda.exe")) { Install-Zluda; }
    
    Write-Host "[GHOST] Initialization complete." -ForegroundColor Green;
    Start-Sleep -Seconds 2;
}

# ==============================================================================
# PHASE 3: MEDIA & DOOM (WINDOWS NATIVE)
# ==============================================================================

$global:MediaPlayer = $null;
$global:DoomProcess = $null;
$global:MusicDisabled = $false;

function Stop-Music {
    if ($global:MediaPlayer) { $global:MediaPlayer.controls.stop(); }
}

function Toggle-Music {
    if ($global:MusicDisabled) { return; }

    if ($null -eq $global:MediaPlayer) {
        $job = Start-Job -ScriptBlock { New-Object -ComObject WMPlayer.OCX };
        $result = Wait-Job $job -Timeout 1;
        if ($result.State -ne "Completed") {
            Stop-Job $job;
            $global:MusicDisabled = $true;
            return;
        }
        $global:MediaPlayer = Receive-Job $job;
        Remove-Job $job;
        
        # FIX 3: WMPlayer "FOCUS STEAL" (Explicitly force the COM object to be invisible)
        if ($global:MediaPlayer) {
            $global:MediaPlayer.uiMode = "invisible";
        }
    }

    if ($global:MediaPlayer.playState -eq 3) {
        Stop-Music;
    } else {
        if (-not (Test-Path $MusicDir)) { New-Item -ItemType Directory -Force -Path $MusicDir | Out-Null; }
        $track = "$MusicDir\Ghost_Track_4.mp3";
        
        if (-not (Test-Path $track) -or (Get-Item $track).Length -lt 100000) {
            Clear-Host;
            Write-Host "[GHOST] Downloading default music..." -ForegroundColor Cyan;
            Invoke-WebRequest -Uri "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-4.mp3" -OutFile $track -ErrorAction SilentlyContinue;
        }
        $global:MediaPlayer.URL = $track;
        $global:MediaPlayer.settings.setMode("loop", $true);
        $global:MediaPlayer.settings.volume = 30;
        $global:MediaPlayer.controls.play();
    }
}

function Launch-Doom {
    $doomExe = Get-ChildItem -Path $DoomDir -Filter "chocolate-doom.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1 | Select-Object -ExpandProperty FullName;
    
    if (-not $doomExe) {
        Clear-Host;
        Write-Host "[GHOST] Downloading Windows DOOM Engine..." -ForegroundColor Cyan;
        New-Item -ItemType Directory -Force -Path $DoomDir -ErrorAction SilentlyContinue | Out-Null;
        
        $zipPath = "$env:TEMP\doom_$PID.zip";
        Invoke-WebRequest -Uri "https://github.com/chocolate-doom/chocolate-doom/releases/download/chocolate-doom-3.0.1/chocolate-doom-3.0.1-win32.zip" -OutFile $zipPath -ErrorAction SilentlyContinue;
        Expand-Archive -Path $zipPath -DestinationPath $DoomDir -Force -ErrorAction SilentlyContinue;
        Remove-Item $zipPath -ErrorAction SilentlyContinue;

        $extractedExe = Get-ChildItem -Path $DoomDir -Filter "chocolate-doom.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1;
        if ($extractedExe) {
            Move-Item -Path "$($extractedExe.DirectoryName)\*" -Destination $DoomDir -Force -ErrorAction SilentlyContinue;
            Remove-Item -Recurse -Force $extractedExe.DirectoryName -ErrorAction SilentlyContinue;
        }

        Write-Host "[GHOST] Downloading DOOM Shareware WAD..." -ForegroundColor Cyan;
        Invoke-WebRequest -Uri "https://archive.org/download/2020_03_22_DOOM/DOOM%20WADs/DOOM1.WAD" -OutFile "$DoomDir\DOOM1.WAD" -ErrorAction SilentlyContinue;
        
        $doomExe = Get-ChildItem -Path $DoomDir -Filter "chocolate-doom.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1 | Select-Object -ExpandProperty FullName;
    }

    if ($doomExe) {
        $global:DoomProcess = Start-Process -FilePath $doomExe -ArgumentList "-iwad `"$DoomDir\DOOM1.WAD`"" -WorkingDirectory (Split-Path $doomExe) -PassThru -WindowStyle Maximized;
    }
}

# ==============================================================================
# PHASE 4: THE WAITING ROOM TUI & SMART FAILOVER
# ==============================================================================

function Read-LogSafely($filePath) {
    # FIX 1: THE "MISSING LOG" CRASH (Silently return if file doesn't exist yet)
    if (-not (Test-Path $filePath)) { return ""; }
    try {
        $fs = [System.IO.File]::Open($filePath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite);
        $reader = New-Object System.IO.StreamReader($fs);
        $content = $reader.ReadToEnd();
        $reader.Close();
        $fs.Close();
        return $content;
    } catch { return ""; }
}

function Get-TrueVRAM {
    $cacheFile = "$ConfigDir\ghost_vram_cache.txt";
    
    if (Test-Path $cacheFile) {
        $fileInfo = Get-Item $cacheFile -ErrorAction SilentlyContinue;
        if ($fileInfo -and $fileInfo.LastWriteTime -gt (Get-Date).AddHours(-24)) {
            return Get-Content $cacheFile -ErrorAction SilentlyContinue;
        }
    }

    $cluster = Get-AmdGpuCluster;
    if ($cluster.Count -gt 0) {
        $totalVramMB = 0;
        foreach ($g in $cluster) {
            if ($g.AdapterRAM -gt 0) {
                $totalVramMB += ($g.AdapterRAM / 1MB);
            }
        }
        
        if ($totalVramMB -gt 0) {
            $vramGB = [math]::Round($totalVramMB / 1024, 1).ToString() + " GB";
            if ($cluster.Count -gt 1) { $vramGB += " (Dual)"; }
            
            if (-not (Test-Path $ConfigDir)) { New-Item -ItemType Directory -Force -Path $ConfigDir -ErrorAction SilentlyContinue | Out-Null; }
            $vramGB | Out-File -FilePath $cacheFile -Encoding utf8 -ErrorAction SilentlyContinue;
            return $vramGB;
        }
    }
    
    return "Loading...";
}

function Get-ProcessPort($pidToFind) {
    try {
        $childPids = (Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.ParentProcessId -eq $pidToFind }).ProcessId;
        $allPids = @($pidToFind) + $childPids;

        $connections = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $allPids -contains $_.OwningProcess };
        if ($connections) {
            return $connections[0].LocalPort;
        }
    } catch {}
    return $null;
}

function Show-WaitingRoom {
    param([int]$TargetPort, [int]$AppPid, [string]$ModeFlag, [string]$LogFile)

    $detectedPort = "Searching...";
    Clear-Host;
    [Console]::CursorVisible = $false;
    
    $startTime = Get-Date;
    
    $padLen = $Host.UI.RawUI.WindowSize.Width - 1;
    if ($padLen -lt 60) { $padLen = 60; }
    $clearSpace = "".PadRight([math]::Max(0, $padLen - 60));

    while ($true) {
        $aiProcess = Get-Process -Id $AppPid -ErrorAction SilentlyContinue;
        if (-not $aiProcess -or $aiProcess.HasExited) { break; }

        $foundPort = Get-ProcessPort -pidToFind $AppPid;
        if ($foundPort) { $detectedPort = $foundPort; }

        $logContent = Read-LogSafely -filePath $LogFile;
        $logReady = $logContent -match "running on local url|uvicorn running on|starting web server|http://(127\.0\.0\.1|0\.0\.0\.0|localhost):[0-9]+";

        if ($foundPort -or $logReady) {
            Stop-Music;
            if ($global:DoomProcess -and -not $global:DoomProcess.HasExited) {
                Stop-Process -Id $global:DoomProcess.Id -Force -ErrorAction SilentlyContinue;
            }
            [Console]::CursorVisible = $true;
            Clear-Host;
            break;
        }
        
        # FIX 2: WAITING ROOM "PORT TRAP" (10-Minute Global Timeout)
        if ((Get-Date) - $startTime -gt [timespan]::FromMinutes(10)) {
            [Console]::CursorVisible = $true;
            Clear-Host;
            Write-Host "`n[GHOST] CRITICAL ERROR: AI Model failed to initialize within 10 minutes." -ForegroundColor Red;
            Write-Host "[GHOST] Possible Shader Compilation Hang or Silent CUDA Error." -ForegroundColor Yellow;
            Stop-Process -Id $AppPid -Force -ErrorAction SilentlyContinue;
            break;
        }

        $trueVram = Get-TrueVRAM;
        $temp = "N/A";

        $spoof = $env:GHOST_SPOOF_NAME.Replace("NVIDIA GeForce ", "");
        $status = "STATUS: BOOTING AI MODEL / PORT: $detectedPort";
        if ($status.Length -gt 54) { $status = $status.Substring(0, 54) + ".."; }

        $backend = if ($ModeFlag -eq "ZLUDA") { "[ BACKEND: Failover Detected -> ZLUDA Active ]" } else { "[ BACKEND: ROCm Native -> Monitoring for failover ]" };

        $status = $status.PadRight(54);
        [Console]::SetCursorPosition(0, 0);
        
        Write-Host ("╔══════════════════════════════════════════════════════════╗" + $clearSpace) -ForegroundColor Cyan;
        Write-Host "║                 " -NoNewline -ForegroundColor Cyan; Write-Host "GHOST ENVIRONMENT (v2.7)" -NoNewline -ForegroundColor Green; Write-Host ("                 ║" + $clearSpace) -ForegroundColor Cyan;
        Write-Host ("╠══════════════════════════════════════════════════════════╣" + $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║  {0,-56}║{1}" -f $status, $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║                                                          ║{0}" -f $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║  {0,-56}║{1}" -f "[ GPU: Spoofed as $spoof ]", $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║  {0,-56}║{1}" -f "[ TEMP: $temp ][ VRAM: $trueVram ]", $clearSpace) -ForegroundColor Cyan;
        
        if ($ModeFlag -eq "ZLUDA") {
            Write-Host "║  " -NoNewline -ForegroundColor Cyan; Write-Host ("{0,-56}" -f $backend) -NoNewline -ForegroundColor Yellow; Write-Host ("║" + $clearSpace) -ForegroundColor Cyan;
        } else {
            Write-Host ("║  {0,-56}║{1}" -f $backend, $clearSpace) -ForegroundColor Cyan;
        }
        
        Write-Host ("╠══════════════════════════════════════════════════════════╣" + $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║  HOTKEYS:                                                ║{0}" -f $clearSpace) -ForegroundColor Cyan;
        Write-Host ("║  {0,-56}║{1}" -f "[D] Play DOOM          [M] Toggle Music", $clearSpace) -ForegroundColor Cyan;
        Write-Host ("╚══════════════════════════════════════════════════════════╝" + $clearSpace) -ForegroundColor Cyan;

        if ([console]::KeyAvailable) {
            $key = [console]::ReadKey($true).Character;
            if ($key -eq 'm' -or $key -eq 'M') { Toggle-Music; }
            if ($key -eq 'd' -or $key -eq 'D') { Launch-Doom; }
        }
        Start-Sleep -Milliseconds 500;
    }
    [Console]::CursorVisible = $true;
}

function Kill-ProcessTree($pidToKill) {
    try {
        $pidsToKill = New-Object System.Collections.Generic.HashSet[int];
        $pidsToKill.Add($pidToKill) | Out-Null;
        
        $newPidsFound = $true;
        while ($newPidsFound) {
            $newPidsFound = $false;
            $allProcesses = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue;
            if ($allProcesses) {
                foreach ($proc in $allProcesses) {
                    if ($pidsToKill.Contains([int]$proc.ParentProcessId) -and -not $pidsToKill.Contains([int]$proc.ProcessId)) {
                        $pidsToKill.Add([int]$proc.ProcessId) | Out-Null;
                        $newPidsFound = $true;
                    }
                }
            }
        }
        
        foreach ($p in $pidsToKill) {
            Stop-Process -Id $p -Force -ErrorAction SilentlyContinue;
            if (Get-Process -Id $p -ErrorAction SilentlyContinue) {
                cmd.exe /c "taskkill /F /T /PID $p >nul 2>&1";
            }
        }
    } catch {}
}

function ghost-start {
    $exe = $args[0];
    
    if (-not (Get-Command $exe -ErrorAction SilentlyContinue)) {
        Write-Host "`n[GHOST] CRITICAL ERROR: Executable '$exe' not found." -ForegroundColor Red;
        Write-Host "[GHOST] Please verify it is installed and in your PATH." -ForegroundColor Yellow;
        return;
    }
    
    $argArray = @();
    if ($args.Length -gt 1) {
        $argArray = $args[1..($args.Length-1)];
    }
    
    $targetPort = if ($env:GHOST_TARGET_PORT) { [int]$env:GHOST_TARGET_PORT } else { 7860 };
    
    $logOut = "$env:TEMP\ghost_ai_out_$PID.log";
    $logErr = "$env:TEMP\ghost_ai_err_$PID.log";
    
    if (Test-Path $logOut) { Remove-Item $logOut -Force -ErrorAction SilentlyContinue; }
    if (Test-Path $logErr) { Remove-Item $logErr -Force -ErrorAction SilentlyContinue; }

    Write-Host "[GHOST] Attempting Native Execution..." -ForegroundColor Cyan;

    $process = $null;
    try {
        $process = Start-Process -FilePath $exe -ArgumentList $argArray -RedirectStandardOutput $logOut -RedirectStandardError $logErr -PassThru -WindowStyle Hidden;
        
        $global:GhostActiveProcessId = $process.Id;
        $global:OnExitJob = Register-ObjectEvent -InputObject ([System.AppDomain]::CurrentDomain) -EventName ProcessExit -Action {
            if ($global:GhostActiveProcessId) {
                cmd.exe /c "taskkill /F /T /PID $($global:GhostActiveProcessId) >nul 2>&1";
            }
        };

        Show-WaitingRoom -TargetPort $targetPort -AppPid $process.Id -ModeFlag "ROCM" -LogFile $logOut;

        $needsFailover = $false;
        if ($process.HasExited) {
            Start-Sleep -Milliseconds 500;
            
            if ($process.ExitCode -ne 0) {
                $errContent = Read-LogSafely -filePath $logErr;
                $outContent = Read-LogSafely -filePath $logOut;
                if ($errContent -match "No HIP backend|CUDA out of memory|Torch not compiled with ROCm" -or $outContent -match "No HIP backend|CUDA out of memory|Torch not compiled with ROCm") {
                    $needsFailover = $true;
                }
            }
        }

        if ($needsFailover) {
            if (-not (Test-Path "$ZludaDir\zluda.exe")) { Install-Zluda; }

            if (Test-Path "$ZludaDir\zluda.exe") {
                Start-Sleep -Seconds 1;
                Remove-Item $logOut, $logErr -ErrorAction SilentlyContinue;
                
                $exeSafe = if ($exe -match "\s") { "`"$exe`"" } else { $exe };
                $zludaArgs = @("--", $exeSafe) + $argArray;
                
                $process = Start-Process -FilePath "$ZludaDir\zluda.exe" -ArgumentList $zludaArgs -RedirectStandardOutput $logOut -RedirectStandardError $logErr -PassThru -WindowStyle Hidden;
                
                Show-WaitingRoom -TargetPort $targetPort -AppPid $process.Id -ModeFlag "ZLUDA" -LogFile $logOut;
            } else {
                Clear-Host;
                Write-Host "[GHOST] CRITICAL: ZLUDA auto-repair failed. Cannot failover." -ForegroundColor Red;
                if (Test-Path $logErr) { Get-Content $logErr; }
                return;
            }
        }

        Clear-Host;
        
        $lastPosOut = 0;
        $lastPosErr = 0;
        while ($process -and -not $process.HasExited) {
            try {
                $fsOut = [System.IO.File]::Open($logOut, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite);
                $readerOut = New-Object System.IO.StreamReader($fsOut);
                $readerOut.BaseStream.Seek($lastPosOut, [System.IO.SeekOrigin]::Begin) | Out-Null;
                $newContentOut = $readerOut.ReadToEnd();
                $lastPosOut = $readerOut.BaseStream.Position;
                $readerOut.Close();
                $fsOut.Close();
                if ($newContentOut) { Write-Host $newContentOut -NoNewline; }

                $fsErr =[System.IO.File]::Open($logErr, [System.IO.FileMode]::Open,[System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite);
                $readerErr = New-Object System.IO.StreamReader($fsErr);
                $readerErr.BaseStream.Seek($lastPosErr, [System.IO.SeekOrigin]::Begin) | Out-Null;
                $newContentErr = $readerErr.ReadToEnd();
                $lastPosErr = $readerErr.BaseStream.Position;
                $readerErr.Close();
                $fsErr.Close();
                if ($newContentErr) { Write-Host $newContentErr -NoNewline -ForegroundColor Red; }
            } catch {}
            Start-Sleep -Milliseconds 250;
        }
        
    } finally {
        if ($global:OnExitJob) {
            Unregister-Event -SourceIdentifier $global:OnExitJob.Name -ErrorAction SilentlyContinue;
            Remove-Job -Name $global:OnExitJob.Name -ErrorAction SilentlyContinue;
        }

        Stop-Music;
        if ($global:DoomProcess -and -not $global:DoomProcess.HasExited) {
            Stop-Process -Id $global:DoomProcess.Id -Force -ErrorAction SilentlyContinue;
        }
        if ($process -and -not $process.HasExited) {
            Write-Host "`n[GHOST] Terminating AI Process Tree..." -ForegroundColor Yellow;
            Kill-ProcessTree -pidToKill $process.Id;
        }
        
        Start-Sleep -Seconds 1;
        
        if ($process -and $process.ExitCode -eq 0) {
            Remove-Item $logOut, $logErr -ErrorAction SilentlyContinue;
        } else {
            Write-Host "`n[GHOST] Process crashed. Full logs preserved at:" -ForegroundColor Yellow;
            Write-Host "OUT: $logOut" -ForegroundColor DarkGray;
            Write-Host "ERR: $logErr" -ForegroundColor DarkGray;
        }
    }
}

# ==============================================================================
# MAIN EXECUTION BLOCK
# ==============================================================================

if ($env:GHOST_ENV_ACTIVE -ne "1") {
    $scriptPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Definition };
    
    if (-not (Test-Path $ConfigFile)) { Run-Wizard; }
    
    Write-Host "[GHOST] Initializing Windows Virtual Environment..." -ForegroundColor Cyan;
    
    [Environment]::SetEnvironmentVariable("GHOST_ENV_ACTIVE", "1", "Process");
    
    $machinePolicy = Get-ExecutionPolicy -Scope MachinePolicy -ErrorAction SilentlyContinue;
    $userPolicy = Get-ExecutionPolicy -Scope UserPolicy -ErrorAction SilentlyContinue;
    
    if ($machinePolicy -in @('Restricted', 'AllSigned') -or $userPolicy -in @('Restricted', 'AllSigned')) {
        Write-Host "`n[GHOST] CRITICAL ERROR: PowerShell execution is blocked by Group Policy." -ForegroundColor Red;
        Write-Host "[GHOST] Please open PowerShell as Administrator and run the following command to allow scripts:" -ForegroundColor Yellow;
        Write-Host "`n        Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`n" -ForegroundColor Cyan;
        Write-Host "Press any key to exit..." -ForegroundColor White;
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown");
        exit;
    }
    
    $argList = @("-NoExit", "-ExecutionPolicy", "Bypass", "-Command", "chcp 65001 >`$null; & `"$scriptPath`"");
    
    try {
        Start-Process powershell.exe -ArgumentList $argList -NoNewWindow -ErrorAction Stop;
        exit;
    } catch {
        Write-Host "`n[GHOST] CRITICAL ERROR: Failed to spawn Ghost Environment." -ForegroundColor Red;
        Write-Host "Press any key to exit..." -ForegroundColor White;
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown");
        exit;
    }
} else {
    Set-GhostEnvironment;
    if (Test-Path $ConfigFile) { . $ConfigFile; }
    
    function prompt {
        Write-Host "(ghost-env) " -NoNewline -ForegroundColor Green;
        Write-Host "$pwd> " -NoNewline;
        return " ";
    }
    
    Write-Host "`n[GHOST] -> To run your AI with failover, type: " -NoNewline -ForegroundColor Cyan;
    Write-Host "ghost-start python your_script.py`n" -ForegroundColor White;
}
