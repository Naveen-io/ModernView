# ModernView Root Setup Script
param(
    [switch]$SkipAssociations = $false,
    [switch]$OpenDefaultApps = $false
)

$script = Join-Path $PSScriptRoot "scripts\install.ps1"
& $script -SkipAssociations:$SkipAssociations -OpenDefaultApps:$OpenDefaultApps
