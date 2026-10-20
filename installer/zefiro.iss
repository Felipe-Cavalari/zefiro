; Instalador do Zefiro (Inno Setup 6). Gere com scripts/package.ps1, que passa a versão e a pasta
; dos executáveis: ISCC /DAppVersion=1.0.0 /DBuildDir=..\target\release installer\zefiro.iss
;
; Instala só para o usuário, sem admin. Nunca apaga %APPDATA%\zefiro (configuração e temas) nem os
; logs: nem ao atualizar, nem ao desinstalar.

#ifndef AppVersion
  #error Passe /DAppVersion=<versão> (o scripts/package.ps1 faz isso).
#endif
#ifndef BuildDir
  #define BuildDir "..\target\release"
#endif

[Setup]
AppId={{6F4C2B0E-8A3D-4E71-9C55-2F0B7D9A1E34}
AppName=Zefiro
AppVersion={#AppVersion}
AppVerName=Zefiro {#AppVersion}
AppPublisher=Felipe Cavalari
AppPublisherURL=https://github.com/Felipe-Cavalari/zefiro
AppSupportURL=https://github.com/Felipe-Cavalari/zefiro/issues
VersionInfoVersion={#AppVersion}
PrivilegesRequired=lowest
DefaultDirName={autopf}\Zefiro
DisableProgramGroupPage=yes
DisableDirPage=auto
OutputDir=..\dist
OutputBaseFilename=Zefiro-{#AppVersion}-setup
SetupIconFile=..\assets\icons\app.ico
UninstallDisplayIcon={app}\zefiro.exe
UninstallDisplayName=Zefiro
LicenseFile=..\LICENSE-MIT
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; O Zefiro é fechado pelo caminho limpo no [Code]; o Restart Manager poderia matá-lo e deixar a
; área da AppBar presa.
CloseApplications=no

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "autostart"; Description: "Iniciar o Zefiro junto com o Windows"

[Files]
Source: "{#BuildDir}\zefiro.exe"; DestDir: "{app}"; Flags: ignoreversion
; A ilha C++ só é usada com [island] engine = "externa".
Source: "{#BuildDir}\zefiro-island.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Zefiro"; Filename: "{app}\zefiro.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Zefiro"; ValueData: """{app}\zefiro.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\zefiro.exe"; Description: "Abrir o Zefiro agora"; Flags: nowait postinstall skipifsilent
; Atualização pelo menu do Z (instalação silenciosa): o Zefiro saiu para o instalador e volta aqui.
Filename: "{app}\zefiro.exe"; Flags: nowait; Check: WizardSilent

[Code]
const
  WM_CLOSE = $0010;

{ Fecha o Zefiro como o menu "Sair": WM_CLOSE na barra, que tira as AppBars da tela. }
procedure CloseZefiro;
var
  Bar: HWND;
  Waited: Integer;
begin
  Bar := FindWindowByClassName('ZefiroBar');
  if Bar = 0 then
    Exit;
  PostMessage(Bar, WM_CLOSE, 0, 0);
  Waited := 0;
  while (FindWindowByClassName('ZefiroBar') <> 0) and (Waited < 60) do
  begin
    Sleep(100);
    Waited := Waited + 1;
  end;
  { A ilha externa leva até 3 s para fechar depois da barra. }
  Sleep(500);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseZefiro;
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseZefiro;
  Result := True;
end;

{ "Iniciar com o Windows" também pode ser ligado pelo menu do Zefiro; desinstalar sempre tira. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Zefiro');
end;
