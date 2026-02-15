import QtQuick 2.15
import QtQuick.Layouts 1.15
import org.kde.plasma.core as PlasmaCore
import org.kde.kirigami as Kirigami

MouseArea {
    id: compactRoot

    readonly property int barWidth: 5
    readonly property int iconSz: Math.min(height - 4, 16)
    readonly property int spacing: 2
    readonly property int margins: 2

    // Tell the panel how wide we need to be
    implicitWidth: {
        var w = margins * 2
        if (root.showCursor) {
            w += iconSz + spacing + barWidth + spacing + barWidth
        }
        if (root.showCursor && root.showClaude) {
            w += spacing + 4 + spacing  // gap
        }
        if (root.showClaude) {
            w += iconSz + spacing + barWidth + spacing + barWidth
        }
        return w
    }

    hoverEnabled: true
    onClicked: root.expanded = !root.expanded

    // Horizontal layout: [icon][bars] [gap] [icon][bars]
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.rightMargin: 2
        anchors.topMargin: 2
        anchors.bottomMargin: 2
        spacing: 2

        // ---- Cursor group ----
        Image {
            visible: root.showCursor
            Layout.preferredWidth: compactRoot.iconSz
            Layout.preferredHeight: compactRoot.iconSz
            Layout.alignment: Qt.AlignVCenter
            source: "cursor-logo.svg"
            sourceSize: Qt.size(compactRoot.iconSz, compactRoot.iconSz)
            fillMode: Image.PreserveAspectFit
        }

        // Cursor Plan bar
        Rectangle {
            visible: root.showCursor
            Layout.preferredWidth: compactRoot.barWidth
            Layout.fillHeight: true
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 2
                height: {
                    var pct = root.cursorPlanPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#818cf8"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 2; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        // Cursor On-Demand bar
        Rectangle {
            visible: root.showCursor
            Layout.preferredWidth: compactRoot.barWidth
            Layout.fillHeight: true
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 2
                height: {
                    var pct = root.cursorOnDemandPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#22c55e"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 2; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        // Gap between providers
        Item {
            visible: root.showCursor && root.showClaude
            Layout.preferredWidth: 4
            Layout.fillHeight: true
        }

        // ---- Claude group ----
        Image {
            visible: root.showClaude
            Layout.preferredWidth: compactRoot.iconSz
            Layout.preferredHeight: compactRoot.iconSz
            Layout.alignment: Qt.AlignVCenter
            source: "claude-logo.svg"
            sourceSize: Qt.size(compactRoot.iconSz, compactRoot.iconSz)
            fillMode: Image.PreserveAspectFit
        }

        // Claude 5h bar
        Rectangle {
            visible: root.showClaude
            Layout.preferredWidth: compactRoot.barWidth
            Layout.fillHeight: true
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 2
                height: {
                    var pct = root.claudeSessionPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#facc15"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 2; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        // Claude Weekly bar
        Rectangle {
            visible: root.showClaude
            Layout.preferredWidth: compactRoot.barWidth
            Layout.fillHeight: true
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 2
                height: {
                    var pct = root.claudeWeeklyPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#f97316"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 2; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }
    }

    // Error/loading indicator dot
    Rectangle {
        visible: root.loading || root.errorMessage.length > 0
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 1
        width: 4
        height: 4
        radius: 2
        color: root.loading ? "#facc15" : "#ef4444"
    }
}
