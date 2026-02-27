#define AppName "LLM Load Balancer"
#define AppExeName "llmlb.exe"

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

#ifndef BinariesDir
  #define BinariesDir "."
#endif

[Setup]
AppId={{B5064C44-1AB8-4AEA-86A1-0B36FEC7B6B7}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=llmlb
DefaultDirName={localappdata}\Programs\llmlb
DefaultGroupName=llmlb
DisableProgramGroupPage=yes
OutputDir=output
OutputBaseFilename=llmlb-windows-x86_64-setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
CloseApplicationsFilter={#AppExeName}
RestartApplications=no
ChangesEnvironment=yes
SetupIconFile=..\..\assets\icons\llmlb.ico
UninstallDisplayIcon={app}\{#AppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Files]
Source: "{#BinariesDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\LLM Load Balancer"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"
Name: "{group}\Uninstall LLM Load Balancer"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}')); Flags: preservestringtype

[Code]
function NeedsAddPath(Dir: string): Boolean;
var
  PathValue: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', PathValue) then
    PathValue := '';
  Result := Pos(';' + UpperCase(Dir) + ';', ';' + UpperCase(PathValue) + ';') = 0;
end;
