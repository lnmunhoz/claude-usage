import QtQuick 2.15
import QtQuick.Controls 2.15 as QQC2
import QtQuick.Layouts 1.15
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: fullRoot

    Layout.minimumWidth: root.showCursor && root.showClaude ? 140 : 80
    Layout.minimumHeight: 200
    Layout.preferredWidth: root.showCursor && root.showClaude ? 180 : 110
    Layout.preferredHeight: 320

    spacing: 6

    // ---- Loading / Error indicator ----
    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 14
        visible: root.loading || root.errorMessage.length > 0

        Text {
            anchors.centerIn: parent
            text: root.loading ? "Loading..." : root.errorMessage
            font.pixelSize: 9
            color: root.loading ? Kirigami.Theme.disabledTextColor : "#ef4444"
            elide: Text.ElideRight
            width: parent.width - 12
            horizontalAlignment: Text.AlignHCenter
        }
    }

    // ---- All providers side by side ----
    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.leftMargin: 8
        Layout.rightMargin: 8
        spacing: 4

        // ---- Cursor group ----
        ColumnLayout {
            visible: root.showCursor
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 2

            // Provider label
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: "Cursor"
                font.pixelSize: 10
                font.bold: true
                color: "#818cf8"
            }

            // Plan label
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: (root.cursorMembershipType || "---").toUpperCase()
                font.pixelSize: 8
                font.letterSpacing: 1
                color: Kirigami.Theme.disabledTextColor
            }

            // Bars
            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.alignment: Qt.AlignHCenter
                spacing: 6

                UsageBar {
                    Layout.fillHeight: true
                    percent: root.cursorPlanPercent
                    tag: "P"
                    barColor: "#818cf8"
                    glowColor: "#818cf8"
                    displayMode: root.displayMode
                }

                UsageBar {
                    Layout.fillHeight: true
                    percent: root.cursorOnDemandPercent
                    tag: "D"
                    barColor: "#22c55e"
                    glowColor: "#22c55e"
                    displayMode: root.displayMode
                }
            }

            // Reset info
            Text {
                Layout.alignment: Qt.AlignHCenter
                visible: root.cursorBillingCycleEnd.length > 0
                text: "Resets " + root.cursorBillingCycleEnd.substring(0, 10)
                font.pixelSize: 8
                color: Kirigami.Theme.disabledTextColor
            }
        }

        // ---- Separator between providers ----
        Rectangle {
            visible: root.showCursor && root.showClaude
            Layout.fillHeight: true
            Layout.topMargin: 8
            Layout.bottomMargin: 24
            width: 1
            color: Kirigami.Theme.separatorColor
            opacity: 0.4
        }

        // ---- Claude group ----
        ColumnLayout {
            visible: root.showClaude
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 2

            // Provider label
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: "Claude"
                font.pixelSize: 10
                font.bold: true
                color: "#f97316"
            }

            // Plan label
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: (root.claudePlanType || "---").toUpperCase()
                font.pixelSize: 8
                font.letterSpacing: 1
                color: Kirigami.Theme.disabledTextColor
            }

            // Bars
            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.alignment: Qt.AlignHCenter
                spacing: 6

                UsageBar {
                    Layout.fillHeight: true
                    percent: root.claudeSessionPercent
                    tag: "5h"
                    barColor: "#facc15"
                    glowColor: "#facc15"
                    displayMode: root.displayMode
                }

                UsageBar {
                    Layout.fillHeight: true
                    percent: root.claudeWeeklyPercent
                    tag: "Wk"
                    barColor: "#f97316"
                    glowColor: "#f97316"
                    displayMode: root.displayMode
                }
            }

            // Reset info
            Text {
                Layout.alignment: Qt.AlignHCenter
                visible: root.claudeSessionReset.length > 0
                text: "5h: " + fullRoot.formatReset(root.claudeSessionReset)
                font.pixelSize: 8
                color: Kirigami.Theme.disabledTextColor
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                visible: root.claudeWeeklyReset.length > 0
                text: "Wk: " + fullRoot.formatReset(root.claudeWeeklyReset)
                font.pixelSize: 8
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }

    // ---- No providers message ----
    Text {
        visible: !root.showCursor && !root.showClaude
        Layout.alignment: Qt.AlignHCenter
        Layout.fillHeight: true
        text: "No providers enabled.\nRight-click to configure."
        font.pixelSize: 10
        color: Kirigami.Theme.disabledTextColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    function formatReset(isoString) {
        if (!isoString) return ""
        try {
            var d = new Date(isoString)
            var now = new Date()
            var diffMs = d.getTime() - now.getTime()
            if (diffMs <= 0) return "now"
            var diffMin = Math.floor(diffMs / 60000)
            if (diffMin < 60) return diffMin + "m"
            var diffHr = Math.floor(diffMin / 60)
            var remMin = diffMin % 60
            return diffHr + "h " + remMin + "m"
        } catch (e) {
            return isoString.substring(0, 16)
        }
    }
}
