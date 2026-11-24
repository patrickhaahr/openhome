var NORMAL_MODE = "cinema";
var WARM_MODE = "expert2";
var UNKNOWN = {state: "idle", value: null, failed: false};

function pending(profile) { return profile.state !== "idle" && profile.state !== "reading"; }

function readable(mode) { return typeof mode === "string" && mode.length > 0; }

function change(profile, changes) {
    var next = {}, key;
    for (key in profile) next[key] = profile[key];
    for (key in changes) next[key] = changes[key];
    return next;
}

function settle(value, failed) { return {state: "idle", value: value, failed: failed}; }

function answer(profile, mode) {
    if (profile.state === "writing") return change(profile, {state: "verifying", token: null});
    if (!readable(mode)) return settle(null, true);
    var warm = mode === WARM_MODE;
    if (profile.state === "verifying") return settle(warm, mode !== profile.target);
    if (profile.state === "reading") return settle(warm, false);
    return change(profile, {state: "writing", token: null, target: warm ? NORMAL_MODE : WARM_MODE, value: warm});
}

function transition(profile, event) {
    switch (event.type) {
    case "refresh":
        return pending(profile) ? profile : change(profile, {state: "reading", token: null});
    case "toggle":
        return pending(profile) || profile.value === null ? profile :
            change(profile, {state: "toggling", token: null, failed: false});
    case "sent":
        return profile.token === null ? change(profile, {token: event.token}) : profile;
    case "reply":
        if (profile.state === "idle" || event.token !== profile.token) return profile;
        return event.accepted ? answer(profile, event.mode) : settle(null, true);
    case "timeout":
        return profile.state === "idle" ? profile : settle(null, true);
    case "observed":
        return pending(profile) || !readable(event.mode) ? profile :
            change(profile, {value: event.mode === WARM_MODE, failed: false});
    }
    return profile;
}

function request(profile) {
    if (profile.token !== null) return null;
    return profile.state === "writing" ?
        {method: "/setSystemSettings", payload: {category: "picture", settings: {pictureMode: profile.target}}} :
        {method: "/getSystemSettings", payload: {category: "picture", keys: ["pictureMode"]}};
}

function abandoned(profile, event, next) {
    var settled = event.type === "reply";
    return settled || profile.state === "idle" || next.token === profile.token ? null : profile.token;
}

function dispatch(setting, event, service, timeout) {
    var previous = setting.profile;
    var next = transition(previous, event);
    if (next === previous) return;
    var stale = abandoned(previous, event, next);
    setting.profile = next;
    if (stale !== null) service.cancel(stale);
    if (next.state === "idle") { timeout.stop(); return; }
    var needed = request(next);
    if (needed === null) return;
    var token = service.call("luna://com.webos.settingsservice", needed.method, JSON.stringify(needed.payload));
    setting.profile = transition(setting.profile, {type: "sent", token: token});
    timeout.restart();
}

function parse(payload) {
    try {
        var data = JSON.parse(payload);
        return {accepted: !!data.returnValue, mode: data.settings && data.settings.pictureMode};
    } catch (e) { return {accepted: false}; }
}
