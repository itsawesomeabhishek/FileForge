# Dev helper: performs a real Explorer file drag with modifier keys held so
# the wheel can be exercised end to end. It drives the real mouse, so it
# first brings the Explorer window to the front and refuses to press the
# button unless an Explorer window is really under the file.
#
#   powershell -File scripts/sim-drag.ps1 -File C:\path\sample.png `
#       [-Keys shift] [-HoldMs 4000] [-DropDx 0 -DropDy -112] [-NoDrop]
#
# -Keys is a comma list of shift,alt,ctrl held during the drag.
# After the wheel opens the cursor is moved by (DropDx, DropDy) from the
# point where the wheel appeared, then the button is released (unless
# -NoDrop, which holds the drag for -HoldMs and then releases it where it is).
param(
  [Parameter(Mandatory = $true)][string]$File,
  [string]$Keys = "shift",
  [int]$HoldMs = 3000,
  [int]$DropDx = 0,
  [int]$DropDy = -112,
  [switch]$NoDrop
)

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Sim {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int max);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  public struct POINT { public int X; public int Y; }
  public static string RootClassAt(int x, int y) {
    POINT p; p.X = x; p.Y = y;
    IntPtr h = WindowFromPoint(p);
    if (h == IntPtr.Zero) return "";
    IntPtr r = GetAncestor(h, 2);
    var sb = new StringBuilder(256);
    GetClassName(r == IntPtr.Zero ? h : r, sb, 256);
    return sb.ToString();
  }
}
"@

$VK = @{ shift = 0x10; ctrl = 0x11; alt = 0x12 }
$held = @()

function Release-All {
  [Sim]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
  foreach ($k in $held) { [Sim]::keybd_event($VK[$k], 0, 2, [UIntPtr]::Zero) }
}

function Move-To([int]$x, [int]$y) {
  # Small steps with a nudge so Explorer sees genuine mouse movement.
  $p = New-Object Sim+POINT
  [void][Sim]::GetCursorPos([ref]$p)
  $steps = 12
  for ($i = 1; $i -le $steps; $i++) {
    $nx = [int]($p.X + ($x - $p.X) * $i / $steps)
    $ny = [int]($p.Y + ($y - $p.Y) * $i / $steps)
    [void][Sim]::SetCursorPos($nx, $ny)
    [Sim]::mouse_event(0x0001, 1, 0, 0, [UIntPtr]::Zero)
    [Sim]::mouse_event(0x0001, -1, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 25
  }
}

$dir = Split-Path $File -Parent
$name = Split-Path $File -Leaf
Start-Process explorer.exe $dir

$root = [System.Windows.Automation.AutomationElement]::RootElement
$item = $null
for ($i = 0; $i -lt 40 -and -not $item; $i++) {
  Start-Sleep -Milliseconds 300
  $byName = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
  $isItem = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)
  $cond = New-Object System.Windows.Automation.AndCondition($byName, $isItem)
  $item = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
if (-not $item) { throw "could not find $name in Explorer" }

# Bring the owning Explorer window to the front (an Alt tap lets
# SetForegroundWindow through the focus-stealing guard).
$window = $item
while ($window -and $window.Current.ControlType.ProgrammaticName -ne "ControlType.Window") {
  $window = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($window)
}
if ($window) {
  $hwnd = [IntPtr]$window.Current.NativeWindowHandle
  [Sim]::ShowWindow($hwnd, 9) | Out-Null
  [Sim]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [Sim]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
  [Sim]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 600
}

$rect = $item.Current.BoundingRectangle
$fx = [int]($rect.X + $rect.Width / 2)
$fy = [int]($rect.Y + $rect.Height / 2)
$class = [Sim]::RootClassAt($fx, $fy)
Write-Host "item at $fx,$fy under window class '$class'"
if ($class -ne "CabinetWClass") { throw "Explorer is not under the file (found '$class'); not touching the mouse" }

try {
  Move-To $fx $fy
  foreach ($k in $Keys.Split(",", [StringSplitOptions]::RemoveEmptyEntries)) {
    $k = $k.Trim(); $held += $k
    [Sim]::keybd_event($VK[$k], 0, 0, [UIntPtr]::Zero)
  }
  Start-Sleep -Milliseconds 150
  [Sim]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)  # left down
  Start-Sleep -Milliseconds 150
  Move-To ($fx + 60) ($fy + 40)
  Start-Sleep -Milliseconds 700
  $p = New-Object Sim+POINT
  [void][Sim]::GetCursorPos([ref]$p)
  Write-Host "wheel centre ~ $($p.X),$($p.Y)  window under cursor: '$([Sim]::RootClassAt($p.X, $p.Y))'"

  if ($NoDrop) {
    Start-Sleep -Milliseconds $HoldMs
  } else {
    Move-To ($p.X + $DropDx) ($p.Y + $DropDy)
    Start-Sleep -Milliseconds 400
  }
} finally {
  Release-All
}
Write-Host "done"
