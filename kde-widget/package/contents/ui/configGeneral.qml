import QtQuick 2.15
import QtQuick.Controls 2.15 as QQC2
import QtQuick.Layouts 1.15
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    id: configPage

    property alias cfg_pollIntervalSeconds: pollIntervalSpinBox.value
    property alias cfg_defaultProvider: providerCombo.currentIndex
    property alias cfg_displayMode: displayModeCombo.currentIndex
    property alias cfg_helperPath: helperPathField.text

    QQC2.SpinBox {
        id: pollIntervalSpinBox
        Kirigami.FormData.label: i18n("Poll interval (seconds):")
        from: 10
        to: 3600
        stepSize: 10
        value: plasmoid.configuration.pollIntervalSeconds
    }

    QQC2.ComboBox {
        id: providerCombo
        Kirigami.FormData.label: i18n("Default provider:")
        model: ["cursor", "claude"]
        currentIndex: model.indexOf(plasmoid.configuration.defaultProvider)
        onCurrentIndexChanged: {
            if (currentIndex >= 0) {
                plasmoid.configuration.defaultProvider = model[currentIndex]
            }
        }
    }

    QQC2.ComboBox {
        id: displayModeCombo
        Kirigami.FormData.label: i18n("Display mode:")
        model: ["remaining", "usage"]
        currentIndex: model.indexOf(plasmoid.configuration.displayMode)
        onCurrentIndexChanged: {
            if (currentIndex >= 0) {
                plasmoid.configuration.displayMode = model[currentIndex]
            }
        }
    }

    QQC2.TextField {
        id: helperPathField
        Kirigami.FormData.label: i18n("Helper script path (leave empty for default):")
        placeholderText: "~/.local/share/token-juice/token_juice_helper.py"
        text: plasmoid.configuration.helperPath
    }
}
