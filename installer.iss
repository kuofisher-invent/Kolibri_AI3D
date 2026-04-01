[Setup]
AppName=Kolibri Ai3D
AppVersion=1.1.0
AppPublisher=Kolibri
AppPublisherURL=https://github.com/kuofisher-invent/Kolibri_AI3D
DefaultDirName={autopf}\Kolibri Ai3D
DefaultGroupName=Kolibri Ai3D
UninstallDisplayIcon={app}\kolibri-cad.exe
OutputDir=dist
OutputBaseFilename=KolibriAi3D_Setup_v1.1.0
SetupIconFile=app\icon.ico
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
DisableProgramGroupPage=yes
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; Main executable
Source: "target\release\kolibri-cad.exe"; DestDir: "{app}"; Flags: ignoreversion
; MCP Server
Source: "target\release\kolibri-mcp-server.exe"; DestDir: "{app}"; Flags: ignoreversion
; Icon
Source: "app\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "app\icon.png"; DestDir: "{app}"; Flags: ignoreversion
; CAD Icons (SVG)
Source: "docs\CAD_icons\*.svg"; DestDir: "{app}\docs\CAD_icons"; Flags: ignoreversion
; SketchUp API DLL (optional, for SKP import/export)
Source: "SketchUpAPI.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
; LibreDWG (DWG↔DXF conversion, bundled — no external dependency needed)
Source: "tools\libredwg\dwg2dxf.exe"; DestDir: "{app}\tools\libredwg"; Flags: ignoreversion
Source: "tools\libredwg\dxf2dwg.exe"; DestDir: "{app}\tools\libredwg"; Flags: ignoreversion
Source: "tools\libredwg\libredwg-0.dll"; DestDir: "{app}\tools\libredwg"; Flags: ignoreversion
Source: "tools\libredwg\libiconv-2.dll"; DestDir: "{app}\tools\libredwg"; Flags: ignoreversion
Source: "tools\libredwg\libpcre2-8-0.dll"; DestDir: "{app}\tools\libredwg"; Flags: ignoreversion

[Icons]
Name: "{group}\Kolibri Ai3D"; Filename: "{app}\kolibri-cad.exe"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uninstall Kolibri Ai3D"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Kolibri Ai3D"; Filename: "{app}\kolibri-cad.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\kolibri-cad.exe"; Description: "Launch Kolibri Ai3D"; Flags: nowait postinstall skipifsilent

[Registry]
; Associate .k3d files
Root: HKA; Subkey: "Software\Classes\.k3d"; ValueType: string; ValueData: "KolibriAi3D.Project"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\KolibriAi3D.Project"; ValueType: string; ValueData: "Kolibri Ai3D Project"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\KolibriAi3D.Project\DefaultIcon"; ValueType: string; ValueData: "{app}\icon.ico"
Root: HKA; Subkey: "Software\Classes\KolibriAi3D.Project\shell\open\command"; ValueType: string; ValueData: """{app}\kolibri-cad.exe"" ""%1"""
