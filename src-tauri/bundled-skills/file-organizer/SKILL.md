---
name: file-organizer
description: 批量整理工作目录中的文件：按类型/日期归类、批量重命名、清理临时文件。
when_to_use: 用户需要整理文件夹、批量重命名、归档文件时
---

# 文件批量整理技能

## 目标
把杂乱的目录整理为按规则组织的结构。

## 常用操作 (PowerShell)

### 按扩展名归类到子文件夹
```powershell
Get-ChildItem -File | Group-Object Extension | ForEach-Object {
  $dir = $_.Name.TrimStart('.'); if (-not $dir) { $dir = 'other' }
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  $_.Group | Move-Item -Destination $dir
}
```

### 按修改日期 (年-月) 归档
```powershell
Get-ChildItem -File | ForEach-Object {
  $folder = $_.LastWriteTime.ToString('yyyy-MM')
  New-Item -ItemType Directory -Force -Path $folder | Out-Null
  Move-Item $_.FullName -Destination $folder
}
```

### 批量重命名 (加前缀)
```powershell
Get-ChildItem -File *.jpg | ForEach-Object -Begin { $i=1 } -Process {
  Rename-Item $_.FullName -NewName ("照片_{0:D3}{1}" -f $i, $_.Extension); $i++
}
```

## 步骤
1. 先用 glob / `Get-ChildItem` 预览目录现状，向用户展示将如何整理。
2. **务必先获得用户确认再执行移动/重命名**（这些是不可逆操作，会触发权限审批）。
3. 执行整理，完成后报告移动了多少文件、新建了哪些文件夹。

## 注意
- 移动/删除属于危险操作，会经过权限审批；批量执行前务必确认规则。
- 遇到同名文件冲突时先提示用户，不要静默覆盖。
