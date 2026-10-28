; The Windows installer, built with Inno Setup 6:
;
;   iscc /DVersion=0.1.0 /DExe=target\release\emerald.exe packaging\windows\emerald.iss
;
; It installs for the current user only (no administrator prompt), so the
; updater can run it silently: with /VERYSILENT it closes the running app,
; replaces it and opens it again.
;
; Signing: pass /DSign and a sign tool named "emerald", for example
;   iscc /DSign "/Semerald=signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /f cert.pfx /p PASSWORD $f" …

#ifndef Version
  #error Pass the version: /DVersion=x.y.z
#endif
#ifndef Exe
  #define Exe "..\..\target\release\emerald.exe"
#endif

[Setup]
; Fixed for good: Windows finds an earlier install by it.
AppId={{6F1D4C2E-8B3A-4E57-9C61-2A7D0E5B9F34}
AppName=Emerald
AppVersion={#Version}
AppVerName=Emerald {#Version}
AppPublisher=KingTimer12
AppPublisherURL=https://github.com/KingTimer12/emerald
DefaultDirName={localappdata}\Programs\Emerald
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Emerald-{#Version}-setup-x86_64
SetupIconFile=..\..\emerald-bin\assets\emerald.ico
UninstallDisplayIcon={app}\emerald.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; The running app is closed to be replaced; the [Run] entry opens it again.
CloseApplications=force
RestartApplications=no
#ifdef Sign
SignTool=emerald
SignedUninstaller=yes
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
english.DiscordLinks=Open discord:// links in Emerald
brazilianportuguese.DiscordLinks=Abrir links discord:// no Emerald

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
; Off by default: the official Discord app registers the same scheme.
Name: "discordlinks"; Description: "{cm:DiscordLinks}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "emerald.exe"; Flags: ignoreversion

[Icons]
; The same AppUserModelID the app gives its windows and notifications.
Name: "{userprograms}\Emerald"; Filename: "{app}\emerald.exe"; AppUserModelID: "Emerald.Desktop"
Name: "{userdesktop}\Emerald"; Filename: "{app}\emerald.exe"; AppUserModelID: "Emerald.Desktop"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\discord"; ValueType: string; ValueName: ""; ValueData: "URL:Discord Protocol"; Flags: uninsdeletekey; Tasks: discordlinks
Root: HKCU; Subkey: "Software\Classes\discord"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Tasks: discordlinks
Root: HKCU; Subkey: "Software\Classes\discord\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\emerald.exe"",0"; Tasks: discordlinks
Root: HKCU; Subkey: "Software\Classes\discord\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\emerald.exe"" ""%1"""; Tasks: discordlinks
; "Open Emerald on login", set from the app's settings, goes with the app.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Emerald"; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "{app}\emerald.exe"; Description: "{cm:LaunchProgram,Emerald}"; Flags: nowait postinstall skipifsilent
; A silent run is an update: open the app again.
Filename: "{app}\emerald.exe"; Flags: nowait skipifnotsilent
