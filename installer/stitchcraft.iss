; Inno Setup script for the Windows installer.
;
; Packages the folder that scripts/stage-windows.ps1 assembles (the exe plus every
; Qt file it needs), so build that first - scripts/build-installer.ps1 does both.
#define MyAppName "Stitchcraft"
#define MyAppExeName "stitchcraft.exe"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif
#ifndef InstallerArch
  #define InstallerArch "x64compatible"
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "stitchcraft-windows-x86_64-setup"
#endif
#ifndef StageDir
  #define StageDir "..\dist\stage"
#endif
#define MyAppPublisher "jbcaleb"
#define MyAppURL "https://github.com/jbcaleb/stitchcraft"

[Setup]
; Identifies this program to Windows across versions, so a newer installer upgrades
; the old install instead of sitting beside it. Never change it once released.
AppId={{6B0E5D5C-2D0B-4C64-9A7B-3F1C8E2A4D91}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
DisableProgramGroupPage=yes
; A silent re-install closes the running app through Restart Manager if it is still
; holding stitchcraft.exe open, and relaunches it afterwards.
CloseApplications=yes
RestartApplications=yes
OutputDir=..\dist
OutputBaseFilename={#OutputBaseFilename}
SetupIconFile=..\res\icons\stitchcraft.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#InstallerArch}
ArchitecturesInstallIn64BitMode={#InstallerArch}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
; Everything in the staged folder: the exe, the Qt DLLs, plugins/ and qml/.
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
