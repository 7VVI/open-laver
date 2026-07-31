---
name: data-wrangling
description: 处理 CSV/Excel 等表格数据：清洗、统计、汇总、格式转换，借助 PowerShell。
when_to_use: 用户需要处理表格、统计数据、转换 CSV/Excel 时
---

# 表格数据处理技能

## 目标
对工作目录中的 CSV / Excel 数据做清洗、统计与汇总。

## 能力与方法
优先使用 PowerShell 内置命令处理，避免依赖未安装的第三方库。

### 读取与预览 CSV
```powershell
Import-Csv .\data.csv | Select-Object -First 10 | Format-Table
```

### 统计与分组汇总
```powershell
$data = Import-Csv .\sales.csv
$data | Group-Object 部门 | ForEach-Object {
  [pscustomobject]@{ 部门=$_.Name; 总额=($_.Group | Measure-Object 金额 -Sum).Sum }
} | Sort-Object 总额 -Descending | Format-Table
```

### 过滤并导出新 CSV
```powershell
Import-Csv .\in.csv | Where-Object { [int]$_.金额 -gt 1000 } | Export-Csv .\out.csv -NoTypeInformation -Encoding UTF8
```

### 处理 Excel (.xlsx)
- 若安装了 ImportExcel 模块：`Import-Excel .\book.xlsx`。
- 否则先让用户用 Excel/WPS 另存为 CSV，再按上面的方法处理。

## 步骤
1. 用 glob / read_file 定位并预览数据文件，确认列名与编码。
2. 与用户确认要做的处理（清洗规则、统计维度、输出格式）。
3. 用 shell 执行 PowerShell 命令处理，注意中文用 `-Encoding UTF8`。
4. 将结果导出为文件，并把关键结论汇总给用户。

## 注意
- 大文件避免一次性全量打印，先取样。
- 修改型操作（导出、覆盖）前确认输出路径，避免误覆盖原始数据。
