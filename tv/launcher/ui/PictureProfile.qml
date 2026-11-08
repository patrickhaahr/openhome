import QtQuick 2.12
import WebOSServices 1.0

Item {
    id: setting
    readonly property string normalMode: "cinema"
    readonly property string warmMode: "expert2"
    property bool active: false
    // Last observed picture mode (true = warm), or null when unknown. Independent of operation phase.
    property var value: null
    property bool failed: false
    // Subscription token (observation channel); independent of the operation below.
    property var subscription: null
    // The single in-flight operation, or null: {kind, token, target}. Exactly one kind at a time:
    //   read    background refresh; token only.
    //   toggle  reading the current mode before a write; token only.
    //   write   setSystemSettings in flight; target is the mode that must be read back.
    //   verify  reading back after the write; success requires the observed mode to equal target.
    // begin() is the only constructor and replaces any previous operation, so a late reply to the
    // old one no longer matches operation.token. finish/fail/mismatch are the only exits.
    property var operation: null
    readonly property bool pending: operation !== null && operation.kind !== "read"
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
    function begin(kind, target, method, payload) {
        operation = {kind: kind, token: service.call("luna://com.webos.settingsservice", method, JSON.stringify(payload)), target: target};
        timeout.restart();
    }
    function beginRead(kind, target) { begin(kind, target, "/getSystemSettings", {category: "picture", keys: ["pictureMode"]}); }
    function refresh() { if (active) watch(); if (!pending) beginRead("read", ""); }
    function toggle() {
        if (pending || value === null) return;
        failed = false; beginRead("toggle", "");
    }
    function finish() { timeout.stop(); operation = null; failed = false; }
    function fail() { timeout.stop(); operation = null; value = null; failed = true; }
    // A completed readback that differs from the requested target: the observation is valid, the toggle failed.
    function mismatch(observed) { timeout.stop(); operation = null; value = observed === warmMode; failed = true; }
    function advance(current, mode) {
        if (current.kind === "write") { beginRead("verify", current.target); return; }
        if (typeof mode !== "string" || mode.length === 0) { fail(); return; }
        if (current.kind === "verify") {
            if (mode === current.target) { value = mode === warmMode; finish(); }
            else mismatch(mode);
            return;
        }
        value = mode === warmMode;
        if (current.kind === "toggle") {
            var target = mode === warmMode ? normalMode : warmMode;
            begin("write", target, "/setSystemSettings", {category: "picture", settings: {pictureMode: target}});
        } else finish();
    }
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
            var current = setting.operation;
            if (current === null || token !== current.token) return;
            timeout.stop();
            var data;
            try { data = JSON.parse(payload); } catch (e) { setting.fail(); return; }
            if (!data.returnValue) { setting.fail(); return; }
            setting.advance(current, data.settings && data.settings.pictureMode);
        }
    }
    Timer { id: timeout; interval: 5000; onTriggered: setting.fail() }
    Timer { interval: 60000; running: setting.active; repeat: true; onTriggered: setting.refresh() }
}
