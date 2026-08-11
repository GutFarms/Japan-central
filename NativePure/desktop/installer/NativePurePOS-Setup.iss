; Native Pure POS — branded Windows install wizard
; Built by CI after `gradlew createDistributable`.
; Requires Inno Setup 6+.

#define MyAppName "Native Pure POS"
#define MyAppVersion "1.14.0"
#define MyAppPublisher "Native Pure"
#define MyAppExeName "NativePureCompanion.exe"
#define MyAppURL "https://github.com/GutFarms/Japan-central"

[Setup]
AppId={{A7C3E91F-4B2D-4E8A-9F1C-6D5B8A0E2C44}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\Native Pure\POS
DefaultGroupName=Native Pure
AllowNoIcons=yes
LicenseFile=
InfoBeforeFile=
OutputDir=..\dist
OutputBaseFilename=NativePure-POS-Setup
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
WizardImageFile=assets\wizard-modern.bmp
WizardSmallImageFile=assets\wizard-small.bmp
WizardImageStretch=yes
DisableWelcomePage=no
DisableProgramGroupPage=no
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
VersionInfoVersion=1.14.0.0
VersionInfoCompany=Native Pure
VersionInfoDescription=Native Pure dispensary point of sale installer
VersionInfoProductName=Native Pure POS

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel1=Welcome to Native Pure POS Setup
WelcomeLabel2=This wizard will install Native Pure Point of Sale on your computer.%n%nThe register uses your Native Pure logo branding and keeps inventory on this PC's hard drive.%n%nClick Next to continue.
FinishedLabel=Native Pure POS is installed. You can launch it from the Start menu or desktop shortcut.
ClickFinish=Click Finish to exit Setup.

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: checkedonce
Name: "quicklaunchicon"; Description: "Create a &Quick Launch shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
; App image produced by Compose createDistributable
Source: "..\build\compose\binaries\main\app\NativePureCompanion\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Comment: "Native Pure point of sale register"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon; Comment: "Native Pure POS"
Name: "{userappdata}\Microsoft\Internet Explorer\Quick Launch\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: quicklaunchicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch Native Pure POS"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
