import QtQuick 2.15
import QtQuick.Layouts 1.15
import org.kde.plasma.plasmoid 2.0
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    // ---- State ----
    property string currentProvider: plasmoid.configuration.defaultProvider || "cursor"
    property string displayMode: plasmoid.configuration.displayMode || "remaining"
    property bool loading: false
    property string errorMessage: ""

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

    // Claude data
    property real claudeSessionPercent: 0
    property real claudeWeeklyPercent: 0
    property string claudeSessionReset: ""
    property string claudeWeeklyReset: ""
    property string claudePlanType: ""
    property var claudeExtraSpend: null
    property var claudeExtraLimit: null

    // Tooltip
    toolTipMainText: "Token Juice"
    toolTipSubText: {
        if (loading) return "Loading..."
        if (errorMessage) return "Error: " + errorMessage
        if (currentProvider === "cursor") {
            return "Cursor: Plan " + cursorPlanPercent.toFixed(1) + "% | On-demand " + cursorOnDemandPercent.toFixed(1) + "%"
        } else {
            return "Claude: 5h " + claudeSessionPercent.toFixed(1) + "% | Week " + claudeWeeklyPercent.toFixed(1) + "%"
        }
    }

    // ---- Helper path resolution ----
    function helperScript() {
        var custom = plasmoid.configuration.helperPath
        if (custom && custom.length > 0) {
            return custom
        }
        var home = StandardPaths.writableLocation(StandardPaths.HomeLocation)
        return home + "/.local/share/token-juice/token_juice_helper.py"
    }

    // ---- Executable DataSource ----
    Plasma5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []

        onNewData: function(source, data) {
            executable.disconnectSource(source)
            root.loading = false

            var stdout = data["stdout"] || ""
            var stderr = data["stderr"] || ""
            var exitCode = data["exit code"]

            if (exitCode !== 0 || stdout.trim().length === 0) {
                root.errorMessage = stderr || "Helper script failed"
                return
            }

            try {
                var result = JSON.parse(stdout)
                if (!result.ok) {
                    root.errorMessage = result.error || "Unknown error"
                    return
                }

                root.errorMessage = ""
                var d = result.data

                if (result.provider === "cursor") {
                    root.cursorPlanPercent = d.percentUsed || 0
                    root.cursorOnDemandPercent = d.onDemandPercentUsed || 0
                    root.cursorUsedUsd = d.usedUsd || 0
                    root.cursorLimitUsd = d.limitUsd || 0
                    root.cursorRemainingUsd = d.remainingUsd || 0
                    root.cursorOnDemandUsedUsd = d.onDemandUsedUsd || 0
                    root.cursorOnDemandLimitUsd = d.onDemandLimitUsd
                    root.cursorMembershipType = d.membershipType || ""
                    root.cursorBillingCycleEnd = d.billingCycleEnd || ""
                } else if (result.provider === "claude") {
                    root.claudeSessionPercent = d.sessionPercentUsed || 0
                    root.claudeWeeklyPercent = d.weeklyPercentUsed || 0
                    root.claudeSessionReset = d.sessionReset || ""
                    root.claudeWeeklyReset = d.weeklyReset || ""
                    root.claudePlanType = d.planType || ""
                    root.claudeExtraSpend = d.extraUsageSpend
                    root.claudeExtraLimit = d.extraUsageLimit
                }
            } catch (e) {
                root.errorMessage = "Failed to parse response: " + e
            }
        }

        function exec(cmd) {
            executable.connectSource(cmd)
        }
    }

    // ---- Polling ----
    function fetchUsage() {
        root.loading = true
        var cmd = "python3 '" + helperScript() + "' " + root.currentProvider
        executable.exec(cmd)
    }

    Timer {
        id: pollTimer
        interval: (plasmoid.configuration.pollIntervalSeconds || 60) * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.fetchUsage()
    }

    // Re-fetch when provider changes
    onCurrentProviderChanged: {
        fetchUsage()
    }

    // ---- Representations ----
    compactRepresentation: CompactRepresentation {}
    fullRepresentation: FullRepresentation {}

    preferredRepresentation: fullRepresentation
}
