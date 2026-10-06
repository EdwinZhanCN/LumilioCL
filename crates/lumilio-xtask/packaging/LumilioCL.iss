; LumilioCL installer, Inno Setup 6. `cargo xtask package` passes every
; define below; build through it rather than opening this in the IDE.
; AppId never changes once released (assets/icons/PACKAGING.md §0).

#ifndef AppVersion
  #error Build with cargo xtask package, which defines AppVersion and the rest
#endif

[Setup]
AppId={{6927ECC0-2947-4CDB-8209-9B6142815660}
AppName=LumilioCL
AppVersion={#AppVersion}
AppVerName=LumilioCL {#AppVersion}
AppPublisher=LumilioCL
AppPublisherURL={#Homepage}
AppSupportURL={#Homepage}/issues
AppUpdatesURL={#Homepage}/releases
VersionInfoVersion={#NumericVersion}
VersionInfoProductVersion={#NumericVersion}
VersionInfoProductTextVersion={#AppVersion}
; A per-user install needs no administrator; the dialog lets one choose
; all users instead.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\LumilioCL
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBase}
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\lumiliocl.exe
UninstallDisplayName=LumilioCL
CloseApplications=yes
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
#ifdef Sign
; The `lumilio` sign tool is passed with /S; the uninstaller is signed too.
SignTool=lumilio
SignedUninstaller=yes
#endif

[Languages]
#if FileExists(CompilerPath + "Languages\ChineseSimplified.isl")
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
#endif
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\lumiliocl.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\BUILD.txt"; DestDir: "{app}"; Flags: ignoreversion

; The shortcuts carry the AppUserModelID the process sets, so a pinned
; taskbar icon and the running window are the same entry.
[Icons]
Name: "{autoprograms}\LumilioCL"; Filename: "{app}\lumiliocl.exe"; AppUserModelID: "{#AppId}"
Name: "{autodesktop}\LumilioCL"; Filename: "{app}\lumiliocl.exe"; AppUserModelID: "{#AppId}"; Tasks: desktopicon

[Run]
Filename: "{app}\lumiliocl.exe"; Description: "{cm:LaunchProgram,LumilioCL}"; Flags: nowait postinstall skipifsilent
