/* OpenHome TV. App model, launch and Edit Home integration adapted from PearWebOS.
 * Copyright (c) 2026 Arné de Klerk, MIT; see LICENSE-PearWebOS.
 * Native QtQuick 2.12: no browser, external assets or credentials in this UI.
 */
import QtQuick 2.12
import "file:///var/lib/pearwebos/ui" as OH

FocusScope {
    id: home
    focus: true
    readonly property real s: width / 1920
    function r(v) { return Math.round(v * s); }
    readonly property string family: face.status === FontLoader.Ready ? face.name : fontManager.smart_Regular.family
    FontLoader { id: face; source: "file:///var/lib/pearwebos/fonts/FamiljenGrotesk-Regular.ttf" }
    FontLoader { source: "file:///var/lib/pearwebos/fonts/FamiljenGrotesk-SemiBold.ttf" }
    readonly property int columns: 6
    readonly property int tileW: r(262)
    readonly property int tileH: r(158)
    readonly property int gapX: r(30)
    readonly property var source: interfaces.launchPoints ? interfaces.launchPoints.launchPointsModel : null
    readonly property var hiddenApps: [
        "com.webos.app.mediadiscovery", "amazon.alexapr", "amazon.alexa.view",
        "lgchannels.uk", "com.webos.app.lgchannels", "com.webos.app.homeconnect",
        "com.webos.app.sportsteamsettings", "com.lgshop.app35", "com.lgtravel.app",
        "epoxyaisports", "com.lgtvod.app", "webos.day", "com.twin.app.audiochannel",
        "com.twin.app.gamingportal", "com.pirate.refresh", "org.webosbrew.inputhook",
        "org.aabytt.webos.custom-screensaver-aerial", "com.amazoff.patcher"
    ]
    property var apps: []
    property var dockApps: []
    property bool drawer: false
    property bool agentOpen: false
    property bool reducedMotion: false
    property string region: "apps"
    property int appIndex: 0
    property int controlIndex: 0
    property var now: new Date()
    property var live: ({day: "", weather: {state: "unavailable", temperature: null, code: null, high: null, low: null, rain: null, wind: null}, recovery: {state: "unavailable", charge: null, rest: null, effort: null, freshness: "unknown"}})
    property bool reading: false
    property var readRequest: null
    property var commandRequest: null
    property string pending: ""
    property string acknowledged: ""
    property string failed: ""
    property string feedback: ""
    readonly property var commands: ["lights-on", "lights-off", "speaker-bluetooth", "speaker-optical"]
    readonly property var labels: ["Lights on", "Lights off", "Bluetooth", "Optical"]
    readonly property var glyphs: ["bulb", "bulb-off", "bluetooth", "optical"]
    readonly property bool today: live.day === Qt.formatDateTime(now, "yyyy-MM-dd")
    readonly property var weather: today && (!live.weather.fetchedAt || now.getTime() - Date.parse(live.weather.fetchedAt) < 21600000) ? live.weather : ({state: "unavailable", temperature: null, code: null, high: null, low: null, rain: null, wind: null})
    readonly property bool weatherStale: weather.state === "stale" || (weather.validUntil && Date.parse(weather.validUntil) <= now.getTime())
    readonly property var recovery: today ? live.recovery : ({state: "unavailable", charge: null, rest: null, effort: null, freshness: "unknown"})

    function rebuild() {
        var next = [];
        if (source) for (var i = 0; i < source.count; i++) {
            var e = source.get(i).entry;
            if (e && e.id && hiddenApps.indexOf(e.id) === -1) next.push(e);
        }
        apps = next; dockApps = next.slice(0, columns);
        appIndex = Math.max(0, Math.min(appIndex, (drawer ? apps : dockApps).length - 1));
    }
    function takeFocus() { forceActiveFocus(); }
    function launch(e) { if (e) interfaces.launchPoints.doLaunch(e); }
    function appTitle(e) {
        if (!e) return "";
        if (/^YouTube/.test(e.title)) return "YouTube";
        if (/^Spotify/.test(e.title)) return "Spotify";
        if (/^Twitch/.test(e.title)) return "Twitch";
        return e.title;
    }
    function openDrawer() { drawer = true; region = "apps"; takeFocus(); }
    function closeDrawer() { drawer = false; appIndex = Math.min(appIndex, dockApps.length - 1); region = "apps"; takeFocus(); }
    function selectApp(i) { appIndex = i; region = "apps"; takeFocus(); }
    function selectControl(i) { controlIndex = i; region = "controls"; takeFocus(); }
    function activate() {
        if (agentOpen) { agent.speaking = !agent.speaking; return; }
        if (region === "controls") {
            if (controlIndex === 4) agentOpen = true;
            else send(commands[controlIndex]);
        } else launch((drawer ? apps : dockApps)[appIndex]);
    }
    function navigate(key) {
        if (agentOpen) return;
        if (region === "controls") {
            if (key === Qt.Key_Left) controlIndex = Math.max(0, controlIndex - 1);
            if (key === Qt.Key_Right) controlIndex = Math.min(4, controlIndex + 1);
            if (key === Qt.Key_Down) { region = "apps"; appIndex = Math.min(controlIndex, dockApps.length - 1); }
        } else if (drawer) {
            if (key === Qt.Key_Left) appIndex = Math.max(0, appIndex - 1);
            if (key === Qt.Key_Right) appIndex = Math.min(apps.length - 1, appIndex + 1);
            if (key === Qt.Key_Up) { if (appIndex < columns) closeDrawer(); else appIndex -= columns; }
            if (key === Qt.Key_Down) appIndex = Math.min(apps.length - 1, appIndex + columns);
        } else {
            if (key === Qt.Key_Left) appIndex = Math.max(0, appIndex - 1);
            if (key === Qt.Key_Right) appIndex = Math.min(dockApps.length - 1, appIndex + 1);
            if (key === Qt.Key_Up) { region = "controls"; controlIndex = Math.min(4, appIndex); }
            if (key === Qt.Key_Down) openDrawer();
        }
    }
    function markOffline() {
        var copy = JSON.parse(JSON.stringify(live));
        if (copy.weather.state !== "unavailable") copy.weather.state = "stale";
        if (copy.recovery.state !== "unavailable") copy.recovery.state = "stale";
        live = copy;
    }
    function refresh() {
        if (reading) return;
        reading = true;
        var req = new XMLHttpRequest(); readRequest = req;
        req.open("GET", "http://127.0.0.1:39479/state");
        req.setRequestHeader("X-OpenHome-TV", "1");
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE || readRequest !== req) return;
            readTimeout.stop(); reading = false; readRequest = null;
            try {
                if (req.status !== 200) throw new Error("offline");
                var data = JSON.parse(req.responseText);
                if (!data.day || !data.weather || !data.recovery) throw new Error("invalid");
                live = data;
            } catch (e) { markOffline(); }
        };
        readTimeout.restart(); req.send();
    }
    function send(command) {
        if (pending) return;
        feedbackTimer.stop(); acknowledged = ""; failed = ""; feedback = "Sending…"; pending = command;
        var req = new XMLHttpRequest(); commandRequest = req;
        req.open("POST", "http://127.0.0.1:39479/command/" + command);
        req.setRequestHeader("X-OpenHome-TV", "1");
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE || commandRequest !== req) return;
            commandTimeout.stop(); commandRequest = null; pending = "";
            var ok = false;
            try { ok = req.status === 200 && JSON.parse(req.responseText).state === "accepted"; } catch (e) {}
            if (ok) { acknowledged = command; feedback = "Command accepted"; }
            else { failed = command; feedback = "No answer · command may have been sent"; }
            feedbackTimer.restart();
        };
        commandTimeout.restart(); req.send();
    }
    function temperature(v) { return v === null || v === undefined ? "–" : Math.round(v) + "°"; }
    function weatherName(code) {
        if (code === null || code === undefined) return "Weather unavailable";
        if (code === 0) return "Clear sky";
        if (code === 1) return "Mostly clear";
        if (code === 2) return "Partly cloudy";
        if (code === 3) return "Overcast";
        if ([45, 48].indexOf(code) !== -1) return "Fog";
        if ([51, 53, 55, 56, 57].indexOf(code) !== -1) return "Drizzle";
        if ([61, 63, 65, 66, 67].indexOf(code) !== -1) return "Rain";
        if ([71, 73, 75, 77].indexOf(code) !== -1) return "Snow";
        if ([80, 81, 82].indexOf(code) !== -1) return "Rain showers";
        if ([85, 86].indexOf(code) !== -1) return "Snow showers";
        if ([95, 96, 99].indexOf(code) !== -1) return "Thunderstorms";
        return "Weather unavailable";
    }
    function weatherGlyph(code) {
        return code === 0 || code === 1 ? "sun" : code >= 71 && code <= 77 || code === 85 || code === 86 ? "snow" : code >= 51 ? "rain" : "cloud";
    }
    Component.onCompleted: { rebuild(); refresh(); }
    Connections {
        target: home.source; ignoreUnknownSignals: true
        onCountChanged: rebuildTimer.restart()
        onRowsMoved: rebuildTimer.restart()
        onDataChanged: rebuildTimer.restart()
    }
    Connections {
        target: uiController; ignoreUnknownSignals: true
        onHomeShownChanged: {
            home.now = new Date();
            if (uiController.homeShown) home.refresh();
            else { home.agentOpen = false; holdTimer.stop(); releaseTimer.stop(); home.holding = false; }
        }
    }
    Timer { id: rebuildTimer; interval: 150; onTriggered: home.rebuild() }
    Timer { interval: 5000; running: home.visible && uiController.homeShown; repeat: true; triggeredOnStart: true; onTriggered: home.now = new Date() }
    Timer { interval: 60000; running: home.visible && uiController.homeShown; repeat: true; onTriggered: home.refresh() }
    Timer {
        id: readTimeout; interval: 10000
        onTriggered: { var req = home.readRequest; home.readRequest = null; home.reading = false; if (req) req.abort(); home.markOffline(); }
    }
    Timer {
        id: commandTimeout; interval: 14000
        onTriggered: {
            var req = home.commandRequest; home.commandRequest = null;
            if (req) req.abort(); home.failed = home.pending; home.pending = "";
            home.feedback = "No answer · command may have been sent"; feedbackTimer.restart();
        }
    }
    Timer { id: feedbackTimer; interval: 4500; onTriggered: { home.acknowledged = ""; home.feedback = home.failed ? home.feedback : ""; } }
    property bool holding: false
    Timer {
        id: holdTimer; interval: 700
        onTriggered: {
            if (!home.agentOpen && home.region === "apps") {
                home.holding = false; releaseTimer.stop(); uiController.startEdit("app");
            }
        }
    }
    Timer {
        id: releaseTimer; interval: 150
        onTriggered: { home.holding = false; holdTimer.stop(); home.activate(); }
    }
    Keys.onPressed: {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            releaseTimer.stop();
            if (!holding) { holding = true; holdTimer.restart(); }
            event.accepted = true;
        } else if ([Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].indexOf(event.key) !== -1) {
            navigate(event.key); event.accepted = true;
        } else if (event.key === webOS.Key_webOS_Back || event.key === Qt.Key_Escape) event.accepted = true;
    }
    Keys.onReleased: {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (holding) releaseTimer.restart(); event.accepted = true;
        } else if (event.key === webOS.Key_webOS_Back || event.key === Qt.Key_Escape) {
            if (agentOpen) agentOpen = false; else if (drawer) closeDrawer();
            event.accepted = true;
        }
    }
    Rectangle { anchors.fill: parent; color: "#000000" }
    // Absorb pointer/wheel input so LG's hidden shelves cannot steal focus.
    MouseArea {
        anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.AllButtons
        onWheel: { home.navigate(wheel.angleDelta.y < 0 ? Qt.Key_Down : Qt.Key_Up); wheel.accepted = true; }
        onClicked: home.takeFocus()
    }
    Item {
        anchors.fill: parent; visible: !home.drawer
        OH.TVText { x: r(96); y: r(57); text: "openhome"; font.family: home.family; font.pixelSize: r(36); color: "#ecece6" }
        Rectangle { x: r(260); y: r(85); width: r(8); height: width; radius: width / 2; color: "#ff6a2e" }
        OH.TVText {
            x: r(88); y: r(128); text: Qt.formatDateTime(home.now, "HH:mm")
            color: "#ecece6"; font.family: home.family; font.pixelSize: r(174); font.letterSpacing: -r(7)
        }
        OH.TVText { x: r(98); y: r(326); text: Qt.formatDateTime(home.now, "dddd, d MMMM"); color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(30) }
        // Today lives next to the clock. Every missing measurement stays a dash.
        OH.Glyph { x: r(790); y: r(166); width: r(82); height: width; kind: home.weatherGlyph(home.weather.code); ink: "#ecece6"; visible: home.weatherName(home.weather.code) !== "Weather unavailable" }
        OH.TVText { x: r(902); y: r(137); text: home.temperature(home.weather.temperature); color: "#ecece6"; font.family: home.family; font.pixelSize: r(94) }
        OH.TVText { x: r(793); y: r(264); text: home.weatherName(home.weather.code); color: "#cfd0ca"; font.family: home.family; font.pixelSize: r(30) }
        OH.TVText { x: r(793); y: r(313); text: "Today  " + home.temperature(home.weather.high) + " / " + home.temperature(home.weather.low); color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(27) }
        OH.TVText { x: r(793); y: r(355); text: home.weatherStale ? "Forecast may be out of date" : home.weather.rain === null ? "" : home.weather.rain.toFixed(1) + " mm rain"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(23) }
        Item {
            id: agentKey; x: r(1505); y: r(90); width: r(300); height: r(270)
            Rectangle { anchors.fill: parent; radius: r(36); color: home.region === "controls" && home.controlIndex === 4 ? "#16171a" : "transparent"; border.width: home.region === "controls" && home.controlIndex === 4 ? r(3) : 0; border.color: "#ecece6" }
            OH.AgentMark { anchors.horizontalCenter: parent.horizontalCenter; y: r(3); width: r(204); height: width }
            OH.TVText { anchors.horizontalCenter: parent.horizontalCenter; y: r(218); text: "Meet Ember"; color: "#ecece6"; font.family: home.family; font.pixelSize: r(29) }
            Accessible.role: Accessible.Button
            Accessible.name: "Meet Ember, agent visual concept"
            Accessible.onPressAction: home.agentOpen = true
            MouseArea { anchors.fill: parent; hoverEnabled: true; onEntered: home.selectControl(4); onClicked: { home.agentOpen = true; home.takeFocus(); } }
        }
        // 1920 - 2*96 = 1728. Four 168-wide controls plus 20/40 gaps = 772.
        // Three 184-wide rings + two 44 gaps = 640, aligned to the right edge.
        OH.TVText { x: r(96); y: r(441); text: "Lights"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(25) }
        OH.TVText { x: r(504); y: r(441); text: "Speaker"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(25) }
        Repeater {
            model: 4
            OH.Control {
                x: r(96 + index * 188 + (index >= 2 ? 32 : 0)); y: r(489); width: r(168); height: r(140)
                label: home.labels[index]; glyph: home.glyphs[index]; family: home.family
                selected: home.region === "controls" && home.controlIndex === index
                pending: home.pending === home.commands[index]
                accepted: home.acknowledged === home.commands[index]
                failed: home.failed === home.commands[index]
                onHovered: home.selectControl(index)
                onActivated: { home.selectControl(index); home.send(home.commands[index]); }
            }
        }
        OH.TVText { x: r(96); y: r(648); text: home.feedback; color: home.failed ? "#ff8a7a" : "#9a9ea6"; font.family: home.family; font.pixelSize: r(23) }
        OH.TVText {
            x: r(1184); y: r(408)
            text: home.recovery.state === "unavailable" ? "Today · no recovery data" : home.recovery.state === "stale" ? "Today · last saved data" : home.recovery.freshness === "confirmed" ? "Today" : home.recovery.freshness === "partial" ? "Today · partial data" : "Today · sync unconfirmed"
            color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(23)
        }
        Repeater {
            model: 3
            OH.ScoreRing {
                x: r(1184 + index * 228); y: r(462); width: r(184); height: r(235)
                label: ["Charge", "Rest", "Effort"][index]; glyph: ["charge", "rest", "effort"][index]; family: home.family
                value: [home.recovery.charge, home.recovery.rest, home.recovery.effort][index]
                stale: home.recovery.state === "stale"
            }
        }
        // The only horizon is the dashboard's small orange sun on a quiet line.
        Rectangle { x: r(96); y: r(401); width: r(1728); height: Math.max(1, r(1)); color: "#272825" }
        Rectangle { x: r(96); y: r(397); width: r(9); height: width; radius: width / 2; color: "#ff6a2e" }
        OH.TVText { x: r(96); y: r(715); text: "Your apps"; color: "#cfd0ca"; font.family: home.family; font.pixelSize: r(29) }
        OH.TVText { anchors.right: parent.right; anchors.rightMargin: r(96); y: r(715); text: "All apps  ↓"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(27) }
        MouseArea { x: r(1584); y: r(703); width: r(240); height: r(60); onClicked: home.openDrawer() }
        Row { x: r(99); y: r(784); spacing: home.gapX; Repeater { model: home.dockApps; delegate: appTile } }
        OH.TVText { x: r(96); y: r(1015); text: "Open-Meteo"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(20) }
        OH.TVText { anchors.right: parent.right; anchors.rightMargin: r(96); y: r(1015); text: "↑ Home controls     ↓ All apps     Hold OK to reorder"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(22) }
    }
    Item {
        anchors.fill: parent; visible: home.drawer
        OH.TVText { x: r(96); y: r(62); text: "Your apps"; color: "#ecece6"; font.family: home.family; font.pixelSize: r(44) }
        OH.TVText { anchors.right: parent.right; anchors.rightMargin: r(96); y: r(76); text: "Back  Home"; color: "#9a9ea6"; font.family: home.family; font.pixelSize: r(26) }
        GridView {
            id: grid; x: r(89); y: r(175); width: r(1752); height: r(825)
            cellWidth: r(292); cellHeight: r(260); leftMargin: r(10); topMargin: r(12); bottomMargin: r(30)
            clip: true; interactive: false; model: home.apps; delegate: appTile
            currentIndex: home.appIndex
            highlight: Item {}
            highlightMoveDuration: 0
            preferredHighlightBegin: r(12); preferredHighlightEnd: height - r(240); highlightRangeMode: GridView.ApplyRange
        }
    }
    Component {
        id: appTile
        Item {
            id: tile
            property var app: modelData
            readonly property bool selected: home.region === "apps" && home.appIndex === index
            width: home.tileW; height: home.tileH
            Accessible.role: Accessible.Button
            Accessible.name: app ? app.title : "Application"
            Accessible.onPressAction: home.launch(tile.app)
            Rectangle {
                anchors.fill: parent; radius: r(22); color: "#16171a"
                border.color: tile.selected ? "#ecece6" : "#272825"; border.width: tile.selected ? r(4) : r(1)
                Image {
                    anchors.centerIn: parent; width: r(130); height: width; fillMode: Image.PreserveAspectFit
                    smooth: true; mipmap: true; asynchronous: true
                    source: tile.app ? tile.app.extraLargeIcon || tile.app.largeIcon || tile.app.icon || "" : ""
                }
            }
            OH.TVText {
                y: parent.height + r(16); width: parent.width; horizontalAlignment: Text.AlignHCenter
                text: home.appTitle(tile.app); elide: Text.ElideRight
                color: tile.selected ? "#ecece6" : "#9a9ea6"; font.family: home.family; font.pixelSize: r(27)
            }
            MouseArea {
                anchors.fill: parent; hoverEnabled: true
                onEntered: home.selectApp(index)
                onClicked: home.launch(tile.app)
                onPressAndHold: uiController.startEdit("app")
            }
        }
    }
    OH.AgentScreen { id: agent; anchors.fill: parent; visible: home.agentOpen && uiController.homeShown; family: home.family; reducedMotion: home.reducedMotion }
}
