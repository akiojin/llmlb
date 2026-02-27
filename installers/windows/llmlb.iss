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
function NormalizePathEntry(Value: string): string;
var
  LastChar: string;
begin
  Value := Trim(Value);

  if (Length(Value) >= 2) and (Copy(Value, 1, 1) = '"') and (Copy(Value, Length(Value), 1) = '"') then
    Value := Copy(Value, 2, Length(Value) - 2);

  while Length(Value) > 0 do
  begin
    LastChar := Copy(Value, Length(Value), 1);
    if (LastChar <> '\') and (LastChar <> '/') then
      Break;
    Delete(Value, Length(Value), 1);
  end;

  Result := UpperCase(Value);
end;

function PathContainsDir(PathValue: string; Dir: string): Boolean;
var
  Remaining: string;
  Entry: string;
  Target: string;
  SeparatorPos: Integer;
begin
  Result := False;
  Remaining := PathValue;
  Target := NormalizePathEntry(Dir);

  while True do
  begin
    SeparatorPos := Pos(';', Remaining);
    if SeparatorPos = 0 then
      Entry := Remaining
    else
    begin
      Entry := Copy(Remaining, 1, SeparatorPos - 1);
      Delete(Remaining, 1, SeparatorPos);
    end;

    if NormalizePathEntry(Entry) = Target then
    begin
      Result := True;
      Exit;
    end;

    if SeparatorPos = 0 then
      Break;
  end;
end;

function NeedsAddPath(Dir: string): Boolean;
var
  PathValue: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', PathValue) then
    PathValue := '';
  Result := not PathContainsDir(PathValue, Dir);
end;
