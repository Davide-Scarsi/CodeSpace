$drives = Get-PSDrive -PSProvider FileSystem | Select-Object -ExpandProperty Root
$all = @()
foreach ($root in $drives) {
    try {
        $all += Get-ChildItem -Path $root -Filter tasks.json -File -Recurse -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
    } catch {}
}
$all = $all | Sort-Object -Unique
foreach ($f in $all) {
    Write-Output "FILE: $f"
    try {
        $json = Get-Content -Raw -Path $f | ConvertFrom-Json
        if ($json.tasks) {
            foreach ($t in $json.tasks) {
                $label = $t.label -join ''
                $ttype = $t.type
                $command = $t.command
                $args = if ($t.args) { ($t.args -join ' ') } else { '' }
                $cs_taskType = $null
                $cs_confirm = $null
                if ($t.codeSpace) { $cs_taskType = $t.codeSpace.taskType; $cs_confirm = $t.codeSpace.confirmationRequest }
                Write-Output "  - label: $label"
                Write-Output "    type: $ttype"
                Write-Output "    command: $command"
                Write-Output "    args: $args"
                Write-Output "    codeSpace.taskType: $cs_taskType"
                Write-Output "    codeSpace.confirmationRequest: $cs_confirm"
            }
        } else {
            Write-Output "  (no tasks array)"
        }
    } catch {
        Write-Output "  (failed to parse)"
    }
}
