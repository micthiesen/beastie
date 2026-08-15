; Inno Setup 6 configuration for the verified dist/windows layout.
; Build with: iscc /DPackageRoot=C:\path\to\dist\windows /DOutputDir=C:\path\to\artifacts packaging\windows\Beastie.iss

#ifndef PackageRoot
  #define PackageRoot "dist\windows"
#endif
#ifndef OutputDir
  #define OutputDir "dist\installers"
#endif

[Setup]
AppId={{A4D5F8C2-4D58-4EF2-8C8F-5AA7F6E1C3B9}
AppName=Beastie
AppVersion=1.0.0
AppPublisher=Beastie Project
DefaultDirName={autopf}\Beastie
DefaultGroupName=Beastie
OutputDir={#OutputDir}
OutputBaseFilename=Beastie-windows-setup
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
PrivilegesRequired=lowest
UninstallDisplayIcon={app}\beastie.exe
; Code signing is intentionally external. Set SignTool in the release environment only.

[Files]
Source: "{#PackageRoot}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Beastie"; Filename: "{app}\beastie.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Beastie"; Filename: "{app}\beastie.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\beastie.exe"; Description: "Launch Beastie"; Flags: nowait postinstall skipifsilent
