$drives = Get-PSDrive -PSProvider FileSystem | Select-Object -ExpandProperty Root
foreach ($root in $drives) {
    try {
        Get-ChildItem -Path $root -Filter tasks.json -File -Recurse -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
    } catch {}
}
