; 数据放在安装目录的 data\ 里（见 main.rs 的 windows_data_dir）。
; 卸载时勾了"删除应用数据"才删，升级时不动。
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RmDir /r "$INSTDIR\data"
    RmDir "$INSTDIR"
  ${EndIf}
!macroend
