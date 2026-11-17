#ifndef AppVersion
  #define AppVersion "0.1.12"
#endif
#ifndef AppSource
  #define AppSource "..\..\target\dist"
#endif

[Setup]
AppId={{59DC98EE-A136-40C9-A7BD-EB0CC50E37C2}
AppName=ducko
AppVersion={#AppVersion}
AppPublisher=ducko
AppPublisherURL=https://github.com/duckuments/ducko-md
DefaultDirName={localappdata}\Programs\ducko
DisableDirPage=no
DisableProgramGroupPage=yes
DisableWelcomePage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir=..\..\dist\windows
OutputBaseFilename=ducko-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\branding\app-icon.ico
UninstallDisplayIcon={app}\ducko.exe
WizardStyle=modern light
WizardSmallImageFile=wizard-logo.bmp
WizardSmallImageBackColor=$FFFFFF
WizardImageStretch=no
WizardSizePercent=105,105
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
ChangesAssociations=yes
SetupLogging=yes
#ifdef Signed
SignTool=ducko
SignedUninstaller=yes
SignedUninstallerDir={#AppSource}
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
SelectDirLabel3=Choose an install folder. Your notes are stored separately.
SelectTasksLabel2=Make ducko easier to reach.
FinishedLabel=ducko is ready. A quiet place for your notes.

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#AppSource}\ducko.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AppSource}\vcruntime140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"
Source: "..\..\THIRD_PARTY.md"; DestDir: "{app}"

[Icons]
Name: "{userprograms}\ducko"; Filename: "{app}\ducko.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\ducko"; Filename: "{app}\ducko.exe"; WorkingDir: "{app}"; Tasks: desktopicon

; "Open with" for Markdown files (not the default app): they are edited in place.
[Registry]
Root: HKA; Subkey: "Software\Classes\Ducko.Markdown"; ValueType: string; ValueName: ""; ValueData: "Markdown"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Ducko.Markdown\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\ducko.exe,0"
Root: HKA; Subkey: "Software\Classes\Ducko.Markdown\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ducko.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "Ducko.Markdown"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: string; ValueName: "Ducko.Markdown"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\ducko.exe\SupportedTypes"; ValueType: string; ValueName: ".md"; ValueData: ""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\ducko.exe\SupportedTypes"; ValueType: string; ValueName: ".markdown"; ValueData: ""

[Run]
Filename: "{app}\ducko.exe"; Description: "Launch ducko"; Flags: nowait postinstall skipifsilent
; The in-app updater installs silently with /UPDATE=1 and expects the app back afterwards.
Filename: "{app}\ducko.exe"; Flags: nowait; Check: IsUpdate

[Code]
function IsUpdate: Boolean;
begin
  Result := ExpandConstant('{param:UPDATE|0}') = '1';
end;

procedure InitializeWizard;
begin
  WizardForm.Caption := 'ducko';
end;
