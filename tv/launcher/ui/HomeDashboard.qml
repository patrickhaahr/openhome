import QtQuick 2.12
import "." as OH

Item {
    property var host
    property var profile
    property Component tiles
    readonly property var home: host
    readonly property var pictureProfile: profile
    readonly property Component appTile: tiles
    function r(v) { return home.r(v); }
    anchors.fill: parent; visible: !home.drawer
    // Dashboard proportions: one large clock, today's weather, three rings.
    OH.TVText {
        x: r(84); y: r(32); text: Qt.formatDateTime(home.now, "HH:mm")
        color: "#ecece6"; font.family: home.family; font.pixelSize: r(270); font.letterSpacing: -r(11)
    }
    OH.TVText {
        x: r(96); y: r(350); text: home.danishDate(home.now)
        color: "#ecece6"; font.family: home.family; font.pixelSize: r(40)
    }
    OH.Glyph { x: r(96); y: r(510); width: r(76); height: width; kind: home.weatherGlyph(home.weather.code); ink: "#ecece6" }
    OH.TVText { x: r(194); y: r(470); text: home.temperature(home.weather.temperature); color: "#ecece6"; font.family: home.family; font.pixelSize: r(112) }
    OH.Glyph { x: r(380); y: r(535); width: r(27); height: width; kind: "stale"; ink: "#9a9ea6"; visible: home.weatherStale }
    Repeater {
        model: 3
        OH.ScoreRing {
            x: r(940 + index * 240); y: r(102); width: r(190); height: width
            label: ["Opladning", "Hvile", "Belastning"][index]; glyph: ["charge", "rest", "effort"][index]; family: home.family
            value: [home.recovery.charge, home.recovery.rest, home.recovery.effort][index]
            stale: home.recovery.state === "stale"
        }
    }
    Rectangle { x: r(1700); y: r(96); width: r(124); height: r(240); radius: width / 2; color: "#16171a" }
    Rectangle { x: r(1700); y: r(380); width: r(124); height: r(240); radius: width / 2; color: "#16171a" }
    Repeater {
        model: 4
        OH.Control {
            x: r(1710); y: r([106, 222, 390, 506][index]); width: r(104); height: width
            label: home.labels[index]; glyph: home.glyphs[index]; primary: index === 0
            selected: home.region === "controls" && home.controlIndex === index
            pending: home.pending === home.commands[index]
            accepted: home.acknowledged === home.commands[index]
            failed: home.failed === home.commands[index]
            onHovered: home.selectControl(index)
            onActivated: { home.selectControl(index); home.send(home.commands[index]); }
        }
    }
    Rectangle { x: r(1700); y: r(656); width: r(124); height: width; radius: width / 2; color: "#16171a" }
    OH.Control {
        x: r(1710); y: r(666); width: r(104); height: width
        label: "Varm natteprofil"; glyph: "eye"; primary: pictureProfile.value === true
        enabled: pictureProfile.value !== null
        opacity: enabled ? 1 : 0.4
        selected: home.region === "controls" && home.controlIndex === 5
        pending: pictureProfile.pending; failed: pictureProfile.failed
        onHovered: home.selectControl(5)
        onActivated: { home.selectControl(5); pictureProfile.toggle(); }
    }
    Item {
        id: agentKey; x: r(1110); y: r(372); width: r(350); height: width
        Rectangle {
            anchors.fill: parent; radius: width / 2; color: "transparent"
            border.width: home.region === "controls" && home.controlIndex === 4 ? r(3) : 0
            border.color: "#ecece6"
        }
        OH.AgentMark { anchors.fill: parent }
        Accessible.role: Accessible.Button
        Accessible.name: "Åbn agentens visuelle prøve"
        Accessible.onPressAction: home.agentOpen = true
        MouseArea { anchors.fill: parent; hoverEnabled: true; onEntered: home.selectControl(4); onClicked: { home.agentOpen = true; home.takeFocus(); } }
    }
    Row { x: r(99); y: r(790); spacing: home.gapX; Repeater { model: home.dockApps; delegate: appTile } }
    OH.Glyph { x: r(936); y: r(1006); width: r(48); height: width; kind: "down"; ink: "#9a9ea6" }
    MouseArea {
        x: r(908); y: r(990); width: r(104); height: r(72)
        Accessible.role: Accessible.Button; Accessible.name: "Alle apps"
        onClicked: home.openDrawer()
    }
}
