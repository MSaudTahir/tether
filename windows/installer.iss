; Builds the Tether installer with Inno Setup: iscc /DVersion=1.0.0 /DArch=x64 windows\installer.iss
; Installs for the current user (no admin needed), from windows\build\Tether-<Arch>.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif

[Setup]
AppId=io.github.msaudtahir.Tether
AppName=Tether
AppVersion={#Version}
AppPublisher=Tether
AppPublisherURL=https://github.com/MSaudTahir/tether
DefaultDirName={autopf}\Tether
DefaultGroupName=Tether
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
OutputDir=build
OutputBaseFilename=Tether-{#Version}-windows-{#Arch}-setup
SetupIconFile=Tether.ico
UninstallDisplayIcon={app}\Tether.exe
LicenseFile=..\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "build\Tether-{#Arch}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{autoprograms}\Tether"; Filename: "{app}\Tether.exe"
Name: "{autodesktop}\Tether"; Filename: "{app}\Tether.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Tether.exe"; Description: "Open Tether"; Flags: nowait postinstall skipifsilent
