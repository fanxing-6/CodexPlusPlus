Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define ROOT "..\..\.."

Name "Codex++"
OutFile "${ROOT}\dist\windows\CodexPlusPlus-${VERSION}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Codex++"
InstallDirRegKey HKCU "Software\Codex++" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!define MUI_ICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"
!define MUI_UNICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  File /r "${ROOT}\dist\windows\app\*.*"

  Delete "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk"

  CreateShortcut "$DESKTOP\Codex++.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$DESKTOP\Codex++ 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateDirectory "$SMPROGRAMS\Codex++"
  CreateShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Codex++" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayName" "Codex++"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "Publisher" "BigPizzaV3"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayIcon" "$INSTDIR\codex-plus-plus-manager.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "UninstallString" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  Delete "$DESKTOP\Codex++.lnk"
  Delete "$DESKTOP\Codex++ 管理工具.lnk"
  Delete "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\Codex++\卸载 Codex++.lnk"
  RMDir "$SMPROGRAMS\Codex++"

  Delete "$INSTDIR\codex-plus-plus.exe"
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  Delete "$INSTDIR\codex_flameshot_embedded.dll"
  Delete "$INSTDIR\Qt6*.dll"
  Delete "$INSTDIR\D3Dcompiler_*.dll"
  Delete "$INSTDIR\opengl32sw.dll"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\generic"
  RMDir /r "$INSTDIR\iconengines"
  RMDir /r "$INSTDIR\imageformats"
  RMDir /r "$INSTDIR\networkinformation"
  RMDir /r "$INSTDIR\platforms"
  RMDir /r "$INSTDIR\styles"
  RMDir /r "$INSTDIR\tls"
  RMDir /r "$INSTDIR\translations"
  RMDir /r "$INSTDIR\tools"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++"
  DeleteRegKey HKCU "Software\Codex++"
SectionEnd
