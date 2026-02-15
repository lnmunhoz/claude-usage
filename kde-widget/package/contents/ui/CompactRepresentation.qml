import QtQuick 2.15
import QtQuick.Layouts 1.15
import org.kde.plasma.core as PlasmaCore
import org.kde.kirigami as Kirigami

MouseArea {
    id: compactRoot

    readonly property int iconSize: Math.min(width, height)

    hoverEnabled: true
    onClicked: root.expanded = !root.expanded

    // Mini bars for all enabled providers
    RowLayout {
        anchors.centerIn: parent
        spacing: 1

        // Cursor bars
        Rectangle {
            visible: root.showCursor
            width: Math.max(3, iconSize * 0.15)
            height: iconSize
            radius: 1.5
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 1.5
                height: {
                    var pct = root.cursorPlanPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#818cf8"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 1.5; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        Rectangle {
            visible: root.showCursor
            width: Math.max(3, iconSize * 0.15)
            height: iconSize
            radius: 1.5
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 1.5
                height: {
                    var pct = root.cursorOnDemandPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#22c55e"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 1.5; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        // Small gap between providers
        Item {
            visible: root.showCursor && root.showClaude
            width: 2
            height: 1
        }

        // Claude bars
        Rectangle {
            visible: root.showClaude
            width: Math.max(3, iconSize * 0.15)
            height: iconSize
            radius: 1.5
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 1.5
                height: {
                    var pct = root.claudeSessionPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#facc15"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 1.5; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }

        Rectangle {
            visible: root.showClaude
            width: Math.max(3, iconSize * 0.15)
            height: iconSize
            radius: 1.5
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                radius: 1.5
                height: {
                    var pct = root.claudeWeeklyPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                color: "#f97316"
                Behavior on height { NumberAnimation { duration: 400 } }
            }
            Rectangle { anchors.fill: parent; radius: 1.5; color: Kirigami.Theme.backgroundColor; opacity: 0.3; z: -1 }
        }
    }

    // Error/loading indicator dot
    Rectangle {
        visible: root.loading || root.errorMessage.length > 0
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 2
        width: 5
        height: 5
        radius: 2.5
        color: root.loading ? "#facc15" : "#ef4444"
    }
}
