@echo off
rem Cargo runner: run the built exe with Qt's DLLs on PATH.
rem QMAKE comes from .cargo/config.toml; Qt's bin folder is the one holding it.
if "%QMAKE%"=="" (
  echo QMAKE is not set - copy .cargo\config.toml.example to .cargo\config.toml and edit it. 1>&2
  exit /b 1
)
for %%I in ("%QMAKE%") do set "QT_BIN=%%~dpI"
set "PATH=%QT_BIN%;%PATH%"
%*
