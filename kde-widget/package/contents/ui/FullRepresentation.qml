import QtQuick 2.15
import QtQuick.Controls 2.15 as QQC2
import QtQuick.Layouts 1.15
import org.kde.kirigami as Kirigami
import org.kde.plasma.components 3.0 as PlasmaComponents

ColumnLayout {
    id: fullRoot

    Layout.minimumWidth: 160
    Layout.minimumHeight: 280
    Layout.preferredWidth: 180
    Layout.preferredHeight: 340

    spacing: 8

    // ---- Header: Provider Toggle ----
    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: 8
        Layout.rightMargin: 8
        Layout.topMargin: 4
        spacing: 4

        PlasmaComponents.TabButton {
            id: cursorTab
            text: "Cursor"
            checked: root.currentProvider === "cursor"
            onClicked: root.currentProvider = "cursor"

            Layout.fillWidth: true

            contentItem: Text {
                text: parent.text
                font.pixelSize: 12
                font.bold: parent.checked
                color: parent.checked ? "#818cf8" : Kirigami.Theme.textColor
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }

        PlasmaComponents.TabButton {
            id: claudeTab
            text: "Claude"
            checked: root.currentProvider === "claude"
            onClicked: root.currentProvider = "claude"

            Layout.fillWidth: true

            contentItem: Text {
                text: parent.text
                font.pixelSize: 12
                font.bold: parent.checked
                color: parent.checked ? "#f97316" : Kirigami.Theme.textColor
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
    }

    // ---- Separator ----
    Rectangle {
        Layout.fillWidth: true
        Layout.leftMargin: 8
        Layout.rightMargin: 8
        height: 1
        color: Kirigami.Theme.separatorColor
    }

    // ---- Loading / Error indicator ----
    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 16
        visible: root.loading || root.errorMessage.length > 0

        Text {
            anchors.centerIn: parent
            text: root.loading ? "Loading..." : ("Error: " + root.errorMessage)
            font.pixelSize: 10
            color: root.loading ? Kirigami.Theme.disabledTextColor : "#ef4444"
            elide: Text.ElideRight
            width: parent.width - 16
            horizontalAlignment: Text.AlignHCenter
        }
    }

    // ---- Bars ----
    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.leftMargin: 20
        Layout.rightMargin: 20
        spacing: 16

        // Cursor view
        UsageBar {
            visible: root.currentProvider === "cursor"
            Layout.fillHeight: true
            Layout.fillWidth: true
            percent: root.cursorPlanPercent
            tag: "P"
            barColor: "#818cf8"
            glowColor: "#818cf8"
            displayMode: root.displayMode
        }

        UsageBar {
            visible: root.currentProvider === "cursor"
            Layout.fillHeight: true
            Layout.fillWidth: true
            percent: root.cursorOnDemandPercent
            tag: "D"
            barColor: "#22c55e"
            glowColor: "#22c55e"
            displayMode: root.displayMode
        }

        // Claude view
        UsageBar {
            visible: root.currentProvider === "claude"
            Layout.fillHeight: true
            Layout.fillWidth: true
            percent: root.claudeSessionPercent
            tag: "5h"
            barColor: "#facc15"
            glowColor: "#facc15"
            displayMode: root.displayMode
        }

        UsageBar {
            visible: root.currentProvider === "claude"
            Layout.fillHeight: true
            Layout.fillWidth: true
            percent: root.claudeWeeklyPercent
            tag: "Week"
            barColor: "#f97316"
            glowColor: "#f97316"
            displayMode: root.displayMode
        }
    }

    // ---- Plan Label ----
    Text {
        Layout.alignment: Qt.AlignHCenter
        Layout.bottomMargin: 8
        text: {
            if (root.currentProvider === "cursor") {
                return (root.cursorMembershipType || "cursor").toUpperCase()
            } else {
                return (root.claudePlanType || "claude").toUpperCase()
            }
        }
        font.pixelSize: 10
        font.letterSpacing: 1.5
        font.bold: true
        color: Kirigami.Theme.disabledTextColor
    }

    // ---- Reset / Billing Info ----
    Text {
        Layout.alignment: Qt.AlignHCenter
        Layout.bottomMargin: 4
        visible: text.length > 0
        text: {
            if (root.currentProvider === "cursor") {
                if (root.cursorBillingCycleEnd) {
                    return "Resets: " + root.cursorBillingCycleEnd.substring(0, 10)
                }
                return ""
            } else {
                if (root.claudeSessionReset) {
                    return "5h resets: " + formatReset(root.claudeSessionReset)
                }
                return ""
            }
        }
        font.pixelSize: 9
        color: Kirigami.Theme.disabledTextColor
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
