import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import tech.sparrow-ai.rtpmidi 1.0

ApplicationWindow {
    visible: true
    width: 480
    height: 320
    title: "RTP-MIDI Control"

    // Instance našeho C++ bridge
    RustService {
        id: rustService
        onIsRunningChanged: console.log("Service running state:", isRunning)
        onErrorOccurred: errorDialog.open()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        Label {
            text: "RTP-MIDI to WLED Service"
            font.pixelSize: 24
            font.bold: true
            Layout.alignment: Qt.AlignHCenter
        }

        GridLayout {
            columns: 2
            
            Label { text: "Status:" }
            Label {
                id: statusLabel
                text: rustService.isRunning ? "Running" : "Stopped"
                color: rustService.isRunning ? "green" : "red"
                font.bold: true
            }

            Label { text: "WLED IP:" }
            Label {
                id: ipLabel
                text: rustService.wledIp
            }
        }
        
        RowLayout {
            Layout.fillWidth: true
            Button {
                id: startButton
                text: "Start Service"
                enabled: !rustService.isRunning
                onClicked: rustService.start()
                Layout.fillWidth: true
            }
            Button {
                id: stopButton
                text: "Stop Service"
                enabled: rustService.isRunning
                onClicked: rustService.stop()
                Layout.fillWidth: true
            }
        }
        
        GroupBox {
            title: "WLED Control"
            Layout.fillWidth: true
            
            RowLayout {
                Label { text: "Set Preset:" }
                SpinBox {
                    id: presetSpinBox
                    from: 1
                    to: 255
                    value: 1
                }
                Button {
                    text: "Set"
                    onClicked: rustService.setWledPreset(presetSpinBox.value)
                }
            }
        }
        
        Item { Layout.fillHeight: true } // Spacer
    }

    Dialog {
        id: errorDialog
        title: "Error"
        standardButtons: Dialog.Ok
        modal: true
        Label {
            text: rustService.errorMessage
        }
    }
}
