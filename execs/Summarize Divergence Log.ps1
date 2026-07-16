param(
    [Parameter(Mandatory = $true)]
    [string]$Path
)

try {
    $json = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    $expected = $json.row.expected_motion_state
    $actual = $json.row.actual_motion_state
    if (-not $expected) {
        $expected = "unknown"
    }
    if (-not $actual) {
        $actual = "unknown"
    }
    "Divergence summary: source_frame={0} core_frame={1} player=P{2} kind={3} expected={4} actual={5}" -f `
        $json.source_frame, `
        $json.core_frame, `
        $json.player, `
        $json.kind, `
        $expected, `
        $actual
} catch {
    "Divergence summary unavailable: {0}" -f $_.Exception.Message
}
