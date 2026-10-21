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
    readonly property int gapX: r(30)
    readonly property var source: interfaces.launchPoints ? interfaces.launchPoints.launchPointsModel : null
    readonly property var hiddenApps: [
        "com.webos.app.mediadiscovery", "com.webos.app.discovery", "amazon.alexapr", "amazon.alexa.view",
        "lgchannels.uk", "com.webos.app.lgchannels", "com.webos.app.homeconnect",
        "com.webos.app.sportsteamsettings", "com.lgshop.app35", "com.lgtravel.app",
        "epoxyaisports", "com.lgtvod.app", "webos.day", "com.twin.app.audiochannel",
        "com.twin.app.gamingportal", "com.pirate.refresh",
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
    readonly property var labels: ["Tænd lys", "Sluk lys", "Bluetooth", "Optisk indgang"]
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
    function danishDate(date) {
        var weekdays = ["søndag", "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag"];
        var months = ["januar", "februar", "marts", "april", "maj", "juni", "juli", "august", "september", "oktober", "november", "december"];
        return weekdays[date.getDay()] + " " + date.getDate() + ". " + months[date.getMonth()];
    }
    function openDrawer() { drawer = true; region = "apps"; takeFocus(); }
    function closeDrawer() { drawer = false; appIndex = Math.min(appIndex, dockApps.length - 1); region = "apps"; takeFocus(); }
    function selectApp(i) { appIndex = i; region = "apps"; takeFocus(); }
    function selectControl(i) { controlIndex = i; region = "controls"; takeFocus(); }
    function activate() {
        if (agentOpen) { agent.speaking = !agent.speaking; return; }
        if (region === "controls") {
            if (controlIndex === 4) agentOpen = true;
            else if (controlIndex === 5) blueLight.toggle();
            else send(commands[controlIndex]);
        } else launch((drawer ? apps : dockApps)[appIndex]);
    }
    function navigate(key) {
        if (agentOpen) return;
        if (region === "controls") {
            if (controlIndex === 4) {
                if (key === Qt.Key_Right) controlIndex = 2;
                if (key === Qt.Key_Up) controlIndex = 1;
                if (key === Qt.Key_Down || key === Qt.Key_Left) { region = "apps"; appIndex = Math.min(3, dockApps.length - 1); }
            } else {
                if (key === Qt.Key_Left) controlIndex = 4;
                if (key === Qt.Key_Up) controlIndex = controlIndex === 5 ? 3 : Math.max(0, controlIndex - 1);
                if (key === Qt.Key_Down) {
                    if (controlIndex < 3) controlIndex++;
                    else if (controlIndex === 3) controlIndex = 5;
                    else { region = "apps"; appIndex = dockApps.length - 1; }
                }
            }
        } else if (drawer) {
            if (key === Qt.Key_Left) appIndex = Math.max(0, appIndex - 1);
            if (key === Qt.Key_Right) appIndex = Math.min(apps.length - 1, appIndex + 1);
            if (key === Qt.Key_Up) { if (appIndex < columns) closeDrawer(); else appIndex -= columns; }
            if (key === Qt.Key_Down) appIndex = Math.min(apps.length - 1, appIndex + columns);
        } else {
            if (key === Qt.Key_Left) appIndex = Math.max(0, appIndex - 1);
            if (key === Qt.Key_Right) appIndex = Math.min(dockApps.length - 1, appIndex + 1);
            if (key === Qt.Key_Up) { region = "controls"; controlIndex = appIndex === 5 ? 5 : 4; }
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
    function weatherGlyph(code) {
        if (code === 0 || code === 1) return "sun";
        if (code === 2) return "cloud-sun";
        if (code === 3) return "cloud";
        if (code === 45 || code === 48) return "fog";
        if ([51,53,55,56,57,61,63,65,66,67,80,81,82].indexOf(code) !== -1) return "rain";
        if ([71,73,75,77,85,86].indexOf(code) !== -1) return "snow";
        if ([95,96,99].indexOf(code) !== -1) return "storm";
        return "unknown";
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
    OH.BlueLight { id: blueLight; active: home.visible && uiController.homeShown }
    Rectangle { anchors.fill: parent; color: "#000000" }
    // Absorb pointer/wheel input so LG's hidden shelves cannot steal focus.
    MouseArea {
        anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.AllButtons
        onWheel: { home.navigate(wheel.angleDelta.y < 0 ? Qt.Key_Down : Qt.Key_Up); wheel.accepted = true; }
        onClicked: home.takeFocus()
    }
    Item {
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
        Rectangle { x: r(1700); y: r(380); width: r(124); height: r(304); radius: width / 2; color: "#16171a" }
        Repeater {
            model: 4
            OH.Control {
                x: r(1710); y: r([106, 222, 454, 570][index]); width: r(104); height: width
                label: home.labels[index]; glyph: home.glyphs[index]; primary: index === 0
                selected: home.region === "controls" && home.controlIndex === index
                pending: home.pending === home.commands[index]
                accepted: home.acknowledged === home.commands[index]
                failed: home.failed === home.commands[index]
                onHovered: home.selectControl(index)
                onActivated: { home.selectControl(index); home.send(home.commands[index]); }
            }
        }
        OH.Glyph { x: r(1740); y: r(392); width: r(44); height: width; kind: "speaker"; ink: "#9a9ea6" }
        OH.Control {
            x: r(1726); y: r(700); width: r(72); height: width
            label: "Reducer blåt lys"; glyph: "eye"; primary: blueLight.value === true
            enabled: blueLight.value !== null
            opacity: enabled ? 1 : 0.4
            selected: home.region === "controls" && home.controlIndex === 5
            pending: blueLight.pending; failed: blueLight.failed
            onHovered: home.selectControl(5)
            onActivated: { home.selectControl(5); blueLight.toggle(); }
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
    Item {
        anchors.fill: parent; visible: home.drawer
        GridView {
            id: grid; x: r(89); y: r(175); width: cellWidth * 6 + leftMargin; height: r(825)
            cellWidth: r(292); cellHeight: r(240); leftMargin: r(10); topMargin: r(12); bottomMargin: r(30)
            clip: true; interactive: false; model: home.apps; delegate: appTile
            currentIndex: home.appIndex
            highlight: Item {}
            highlightMoveDuration: 0
            preferredHighlightBegin: r(12); preferredHighlightEnd: height - r(220); highlightRangeMode: GridView.ApplyRange
        }
    }
    Component {
        id: appTile
        Item {
            id: tile
            property var app: modelData
            readonly property bool selected: home.region === "apps" && home.appIndex === index
            width: home.tileW; height: r(188)
            Accessible.role: Accessible.Button
            Accessible.name: app ? app.title : "App"
            Accessible.onPressAction: home.launch(tile.app)
            OH.AppArtwork {
                anchors.fill: parent; cornerRadius: r(22); iconSize: r(154)
                source: tile.app ? tile.app.extraLargeIcon || tile.app.largeIcon || tile.app.icon || "" : ""
            }
            Rectangle {
                anchors.fill: parent; radius: r(22); color: "transparent"
                border.color: "#ecece6"; border.width: tile.selected ? r(4) : 0
            }
            MouseArea {
                anchors.fill: parent; hoverEnabled: true
                onEntered: home.selectApp(index)
                onClicked: home.launch(tile.app)
                onPressAndHold: uiController.startEdit("app")
            }
        }
    }
    OH.AgentScreen { id: agent; anchors.fill: parent; visible: home.agentOpen && uiController.homeShown; reducedMotion: home.reducedMotion; onDismissed: { home.agentOpen = false; home.takeFocus(); } }
}
