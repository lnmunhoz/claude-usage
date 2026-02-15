import QtQuick 2.15
import QtQuick.Layouts 1.15
import Qt5Compat.GraphicalEffects
import org.kde.kirigami as Kirigami

/**
 * UsageBar -- a thin vertical progress bar with gradient color, glow, and label.
 *
 * Properties:
 *   percent     : 0-100 fill value (before display mode adjustment)
 *   tag         : short label below the bar ("P", "D", "5h", "Wk")
 *   barColor    : primary fill color
 *   glowColor   : glow/shadow color
 *   displayMode : "usage" or "remaining"
 *   showTag     : whether to show the tag label (default true)
 */
ColumnLayout {
    id: barRoot

    property real percent: 0
    property string tag: ""
    property color barColor: "#818cf8"
    property color glowColor: barColor
    property string displayMode: "remaining"
    property bool showTag: true

    spacing: 2

    // Effective fill percentage accounting for display mode
    readonly property real fillPercent: {
        var p = Math.max(0, Math.min(100, percent))
        return displayMode === "remaining" ? (100 - p) : p
    }

    // Color that shifts towards red at high usage
    function usageColor(pct, baseColor) {
        if (pct >= 90) return "#ef4444"
        if (pct >= 75) return Qt.tint(baseColor, "#40f97316")
        return baseColor
    }

    // The bar track
    Item {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumWidth: 8
        Layout.maximumWidth: 14
        Layout.minimumHeight: 60
        Layout.alignment: Qt.AlignHCenter

        // Track background
        Rectangle {
            id: track
            anchors.fill: parent
            radius: 4
            color: Kirigami.Theme.backgroundColor
            opacity: 0.25
        }

        // Fill
        Rectangle {
            id: fill
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.height * barRoot.fillPercent / 100
            radius: 4

            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.lighter(usageColor(barRoot.percent, barRoot.barColor), 1.2) }
                GradientStop { position: 1.0; color: usageColor(barRoot.percent, barRoot.barColor) }
            }

            Behavior on height {
                NumberAnimation { duration: 600; easing.type: Easing.InOutQuad }
            }

            // Glow effect
            layer.enabled: true
            layer.effect: DropShadow {
                transparentBorder: true
                horizontalOffset: 0
                verticalOffset: 0
                radius: 8
                samples: 17
                color: Qt.rgba(barRoot.glowColor.r, barRoot.glowColor.g, barRoot.glowColor.b, 0.45)
            }

            // Glass highlight strip
            Rectangle {
                anchors.left: parent.left
                anchors.leftMargin: 1
                anchors.top: parent.top
                anchors.topMargin: 3
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 3
                width: 2
                radius: 1
                color: "#ffffff"
                opacity: 0.2
            }

            // Meniscus
            Rectangle {
                visible: barRoot.fillPercent > 2
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                height: 2
                radius: 1
                color: "#ffffff"
                opacity: 0.3
            }
        }
    }

    // Percentage text
    Text {
        Layout.alignment: Qt.AlignHCenter
        text: barRoot.percent.toFixed(0) + "%"
        font.pixelSize: 9
        font.family: "monospace"
        font.bold: true
        color: Kirigami.Theme.textColor
    }

    // Tag label
    Text {
        visible: barRoot.showTag && barRoot.tag.length > 0
        Layout.alignment: Qt.AlignHCenter
        text: barRoot.tag
        font.pixelSize: 8
        color: Kirigami.Theme.disabledTextColor
    }
}
