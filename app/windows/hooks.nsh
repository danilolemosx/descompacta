; Menu do botão direito no Windows (instalado só para o usuário atual, em HKCU).
; "Compactar em ZIP" em qualquer arquivo ou pasta; "Descompactar aqui" nos compactados.
; No Windows 11, os itens ficam em "Mostrar mais opções".

!macro DESCOMPACTA_VERBO CHAVE TEXTO
  WriteRegStr HKCU "Software\Classes\${CHAVE}" "MUIVerb" "${TEXTO}"
  WriteRegStr HKCU "Software\Classes\${CHAVE}" "Icon" "$INSTDIR\descompacta.exe"
  WriteRegStr HKCU "Software\Classes\${CHAVE}" "MultiSelectModel" "Player"
  WriteRegStr HKCU "Software\Classes\${CHAVE}\command" "" '"$INSTDIR\descompacta.exe" "%1"'
!macroend

!macro DESCOMPACTA_AQUI EXT
  !insertmacro DESCOMPACTA_VERBO "SystemFileAssociations\.${EXT}\shell\DescompactaAqui" "Descompactar aqui"
!macroend

!macro DESCOMPACTA_TIRAR_AQUI EXT
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.${EXT}\shell\DescompactaAqui"
!macroend

!macro DESCOMPACTA_EXTENSOES ACAO
  !insertmacro ${ACAO} "zip"
  !insertmacro ${ACAO} "rar"
  !insertmacro ${ACAO} "7z"
  !insertmacro ${ACAO} "tar"
  !insertmacro ${ACAO} "gz"
  !insertmacro ${ACAO} "tgz"
  !insertmacro ${ACAO} "bz2"
  !insertmacro ${ACAO} "xz"
  !insertmacro ${ACAO} "zst"
  !insertmacro ${ACAO} "cab"
  !insertmacro ${ACAO} "iso"
  !insertmacro ${ACAO} "001"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro DESCOMPACTA_VERBO "*\shell\DescompactaZip" "Compactar em ZIP"
  !insertmacro DESCOMPACTA_VERBO "Directory\shell\DescompactaZip" "Compactar em ZIP"
  !insertmacro DESCOMPACTA_EXTENSOES DESCOMPACTA_AQUI
  ; avisa o Explorer
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\*\shell\DescompactaZip"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\DescompactaZip"
  !insertmacro DESCOMPACTA_EXTENSOES DESCOMPACTA_TIRAR_AQUI
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
