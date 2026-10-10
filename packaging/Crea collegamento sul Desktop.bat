@echo off
rem Creates a "Forma" shortcut on the Desktop and in the Start menu that points to
rem forma.exe in this folder. Run it again after moving the folder.
set "EXE=%~dp0forma.exe"
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$s=New-Object -ComObject WScript.Shell;" ^
  "foreach($d in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))){" ^
  "$l=$s.CreateShortcut((Join-Path $d 'Forma.lnk')); $l.TargetPath=$env:EXE; $l.WorkingDirectory=(Split-Path $env:EXE); $l.Description='Forma - modellatore 3D'; $l.Save()}"
echo Collegamento "Forma" creato sul Desktop e nel menu Start.
pause
