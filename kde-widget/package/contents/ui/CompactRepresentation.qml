import QtQuick 2.15
import QtQuick.Layouts 1.15
import org.kde.plasma.core as PlasmaCore
import org.kde.kirigami as Kirigami

MouseArea {
    id: compactRoot

    readonly property bool isVertical: (plasmoid.formFactor === PlasmaCore.Types.Vertical)
    readonly property int iconSize: Math.min(width, height)

    hoverEnabled: true
    onClicked: root.expanded = !root.expanded

    // Two small bars as the compact icon
    RowLayout {
        anchors.centerIn: parent
        spacing: 2

        Rectangle {
            id: bar1
            width: Math.max(4, iconSize * 0.2)
            height: iconSize
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: {
                    var pct = root.currentProvider === "cursor" ? root.cursorPlanPercent : root.claudeSessionPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                radius: 2
                color: root.currentProvider === "cursor" ? "#818cf8" : "#facc15"

                Behavior on height { NumberAnimation { duration: 400; easing.type: Easing.InOutQuad } }
            }

            // Track background
            Rectangle {
                anchors.fill: parent
                radius: 2
                color: Kirigami.Theme.backgroundColor
                opacity: 0.3
                z: -1
            }
        }

        Rectangle {
            id: bar2
            width: Math.max(4, iconSize * 0.2)
            height: iconSize
            radius: 2
            color: "transparent"

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: {
                    var pct = root.currentProvider === "cursor" ? root.cursorOnDemandPercent : root.claudeWeeklyPercent
                    if (root.displayMode === "remaining") pct = 100 - pct
                    return parent.height * Math.max(0, Math.min(100, pct)) / 100
                }
                radius: 2
                color: root.currentProvider === "cursor" ? "#22c55e" : "#f97316"

                Behavior on height { NumberAnimation { duration: 400; easing.type: Easing.InOutQuad } }
            }

            Rectangle {
                anchors.fill: parent
                radius: 2
                color: Kirigami.Theme.backgroundColor
                opacity: 0.3
                z: -1
            }
        }
    }

    // Error/loading indicator dot
    Rectangle {
        visible: root.loading || root.errorMessage.length > 0
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 2
        width: 6
        height: 6
        radius: 3
        color: root.loading ? "#facc15" : "#ef4444"
    }
}
