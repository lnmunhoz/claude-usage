import QtQuick 2.15
import QtQuick.Layouts 1.15
import org.kde.plasma.plasmoid 2.0
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    // ---- Config bindings ----
    property bool showCursor: plasmoid.configuration.showCursor !== false
    property bool showClaude: plasmoid.configuration.showClaude !== false
    property string displayMode: plasmoid.configuration.displayMode || "remaining"

    // ---- Loading / error per provider ----
    property bool cursorLoading: false
    property bool claudeLoading: false
    property string cursorError: ""
    property string claudeError: ""

    readonly property bool loading: cursorLoading || claudeLoading
    readonly property string errorMessage: {
        var parts = []
        if (cursorError) parts.push("Cursor: " + cursorError)
        if (claudeError) parts.push("Claude: " + claudeError)
        return parts.join(" | ")
    }

    // Cursor data
    property real cursorPlanPercent: 0
    property real cursorOnDemandPercent: 0
    property real cursorUsedUsd: 0
    property real cursorLimitUsd: 0
    property real cursorRemainingUsd: 0
    property real cursorOnDemandUsedUsd: 0
    property var cursorOnDemandLimitUsd: null
    property string cursorMembershipType: ""
    property string cursorBillingCycleEnd: ""
    property bool cursorDataLoaded: false

    // Claude data
    property real claudeSessionPercent: 0
    property real claudeWeeklyPercent: 0
    property string claudeSessionReset: ""
    property string claudeWeeklyReset: ""
    property string claudePlanType: ""
    property var claudeExtraSpend: null
    property var claudeExtraLimit: null
    property bool claudeDataLoaded: false

    // Tooltip
    toolTipMainText: "Token Juice"
    toolTipSubText: {
        if (loading) return "Loading..."
        var parts = []
        if (showCursor && cursorDataLoaded)
            parts.push("Cursor: P " + cursorPlanPercent.toFixed(0) + "% D " + cursorOnDemandPercent.toFixed(0) + "%")
        if (showClaude && claudeDataLoaded)
            parts.push("Claude: 5h " + claudeSessionPercent.toFixed(0) + "% Wk " + claudeWeeklyPercent.toFixed(0) + "%")
        if (errorMessage) parts.push(errorMessage)
        return parts.join(" | ") || "No providers enabled"
    }

    // ---- Executable DataSource ----
    Plasma5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []

        onNewData: function(source, data) {
            var stdout = data["stdout"]
            if (stdout === undefined || stdout === null) return

            executable.disconnectSource(source)

            stdout = stdout.toString().trim()
            var stderr = (data["stderr"] || "").toString().trim()
            var exitCode = data["exit code"]

            // Determine which provider this result is for from the command
            var isCursor = source.indexOf("cursor") !== -1
            var isClaude = source.indexOf("claude") !== -1

            if (isCursor) root.cursorLoading = false
            if (isClaude) root.claudeLoading = false

            if (stdout.length === 0) {
                var errMsg = stderr || ("Helper exited with code " + exitCode)
                if (isCursor) root.cursorError = errMsg
                if (isClaude) root.claudeError = errMsg
                return
            }

            try {
                var result = JSON.parse(stdout)
                if (!result.ok) {
                    if (isCursor) root.cursorError = result.error || "Unknown error"
                    if (isClaude) root.claudeError = result.error || "Unknown error"
                    return
                }

                var d = result.data

                if (result.provider === "cursor") {
                    root.cursorError = ""
                    root.cursorPlanPercent = d.percentUsed || 0
                    root.cursorOnDemandPercent = d.onDemandPercentUsed || 0
                    root.cursorUsedUsd = d.usedUsd || 0
                    root.cursorLimitUsd = d.limitUsd || 0
                    root.cursorRemainingUsd = d.remainingUsd || 0
                    root.cursorOnDemandUsedUsd = d.onDemandUsedUsd || 0
                    root.cursorOnDemandLimitUsd = d.onDemandLimitUsd
                    root.cursorMembershipType = d.membershipType || ""
                    root.cursorBillingCycleEnd = d.billingCycleEnd || ""
                    root.cursorDataLoaded = true
                } else if (result.provider === "claude") {
                    root.claudeError = ""
                    root.claudeSessionPercent = d.sessionPercentUsed || 0
                    root.claudeWeeklyPercent = d.weeklyPercentUsed || 0
                    root.claudeSessionReset = d.sessionReset || ""
                    root.claudeWeeklyReset = d.weeklyReset || ""
                    root.claudePlanType = d.planType || ""
                    root.claudeExtraSpend = d.extraUsageSpend
                    root.claudeExtraLimit = d.extraUsageLimit
                    root.claudeDataLoaded = true
                }
            } catch (e) {
                if (isCursor) root.cursorError = "Parse error: " + e.toString()
                if (isClaude) root.claudeError = "Parse error: " + e.toString()
            }
        }

        function exec(cmd) {
            executable.connectSource(cmd)
        }
    }

    // ---- Helper command builder ----
    function buildCmd(provider) {
        var custom = plasmoid.configuration.helperPath
        if (custom && custom.length > 0) {
            return "'" + custom + "' " + provider
        }
        return "bash -c '\"$HOME/.local/share/token-juice/token-juice-helper\" " + provider + "'"
    }

    // ---- Polling ----
    function fetchAll() {
        if (root.showCursor) {
            root.cursorLoading = true
            executable.exec(buildCmd("cursor"))
        }
        if (root.showClaude) {
            root.claudeLoading = true
            executable.exec(buildCmd("claude"))
        }
    }

    Timer {
        id: pollTimer
        interval: (plasmoid.configuration.pollIntervalSeconds || 60) * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.fetchAll()
    }

    // Re-fetch when provider toggles change
    onShowCursorChanged: fetchAll()
    onShowClaudeChanged: fetchAll()

    // ---- Representations ----
    compactRepresentation: CompactRepresentation {}
    fullRepresentation: FullRepresentation {}

}
