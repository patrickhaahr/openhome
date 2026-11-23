import QtQuick 2.12
import WebOSServices 1.0
import "PictureProfileLifecycle.js" as Lifecycle

Item {
    id: setting
    property bool active: false
    property var subscription: null
    property var profile: Lifecycle.UNKNOWN
    readonly property var value: profile.value
    readonly property bool failed: profile.failed
    readonly property bool pending: Lifecycle.pending(profile)
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
    function dispatch(event) { Lifecycle.dispatch(setting, event, service, timeout); }
    function refresh() { if (active) watch(); dispatch({type: "refresh"}); }
    function toggle() { dispatch({type: "toggle"}); }
    Service {
        id: updates
        appId: "com.webos.app.home"
        onResponse: {
            if (token !== setting.subscription) return;
            var update = Lifecycle.parse(payload);
            if (!update.accepted) { setting.stopWatching(); return; }
            setting.dispatch({type: "observed", mode: update.mode});
        }
    }
    Service {
        id: service
        appId: "com.webos.app.home"
        onResponse: {
            var reply = Lifecycle.parse(payload);
            setting.dispatch({type: "reply", token: token, accepted: reply.accepted, mode: reply.mode});
        }
    }
    Timer { id: timeout; interval: 5000; onTriggered: setting.dispatch({type: "timeout"}) }
    Timer { interval: 60000; running: setting.active; repeat: true; onTriggered: setting.refresh() }
}
