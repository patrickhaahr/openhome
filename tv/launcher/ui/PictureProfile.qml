import QtQuick 2.12
import WebOSServices 1.0

Item {
    id: setting
    readonly property string normalMode: "cinema"
    readonly property string warmMode: "expert2"
    property bool active: false
    property var value: null
    property bool pending: false
    property bool failed: false
    property var request: null
    property var subscription: null
    property string operation: ""
    onActiveChanged: {
        if (active) refresh();
        else stopWatching();
    }
    function stopWatching() {
        var token = subscription;
        subscription = null;
        if (token !== null) updates.cancel(token);
    }
    function watch() {
        if (subscription === null) {
            subscription = updates.call("luna://com.webos.settingsservice", "/getSystemSettings",
                JSON.stringify({category: "picture", keys: ["pictureMode"], subscribe: true}));
        }
    }
    function read() {
        request = service.call("luna://com.webos.settingsservice", "/getSystemSettings",
            JSON.stringify({category: "picture", keys: ["pictureMode"]}));
        timeout.restart();
    }
    function refresh() { if (active) watch(); if (!pending) { operation = "read"; read(); } }
    function toggle() {
        if (pending || value === null) return;
        pending = true; failed = false; operation = "toggle"; read();
    }
    function fail() { request = null; operation = ""; pending = false; value = null; failed = true; timeout.stop(); }
    Service {
        id: updates
        appId: "com.webos.app.home"
        onResponse: {
            if (token !== setting.subscription) return;
            var data;
            try { data = JSON.parse(payload); } catch (e) { setting.stopWatching(); return; }
            if (!data.returnValue) { setting.stopWatching(); return; }
            var current = data.settings && data.settings.pictureMode;
            if (typeof current === "string" && current.length > 0 && !setting.pending) {
                setting.value = current === setting.warmMode;
                setting.failed = false;
            }
        }
    }
    Service {
        id: service
        appId: "com.webos.app.home"
        onResponse: {
            if (token !== setting.request) return;
            timeout.stop();
            var data;
            try { data = JSON.parse(payload); } catch (e) { setting.fail(); return; }
            if (!data.returnValue) { setting.fail(); return; }
            if (setting.operation === "write") {
                setting.operation = "verify"; setting.read(); return;
            }
            var current = data.settings && data.settings.pictureMode;
            if (typeof current !== "string" || current.length === 0) { setting.fail(); return; }
            setting.value = current === setting.warmMode;
            if (setting.operation === "toggle") {
                setting.operation = "write";
                setting.request = service.call("luna://com.webos.settingsservice", "/setSystemSettings",
                    JSON.stringify({category: "picture", settings: {pictureMode: current === setting.warmMode ? setting.normalMode : setting.warmMode}}));
                timeout.restart(); return;
            }
            setting.request = null; setting.operation = ""; setting.pending = false; setting.failed = false;
        }
    }
    Timer { id: timeout; interval: 5000; onTriggered: setting.fail() }
    Timer { interval: 60000; running: setting.active; repeat: true; onTriggered: setting.refresh() }
}
