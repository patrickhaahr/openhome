var UNREAD = {state: "idle", live: {
    day: "",
    weather: {state: "unavailable", temperature: null, code: null, high: null, low: null, rain: null, wind: null},
    recovery: {state: "unavailable", charge: null, rest: null, effort: null, freshness: "unknown"}
}};
var UNSENT = {state: "idle"};
var RELEASED = {state: "idle"};

function offline(live) {
    var copy = JSON.parse(JSON.stringify(live));
    if (copy.weather.state !== "unavailable") copy.weather.state = "stale";
    if (copy.recovery.state !== "unavailable") copy.recovery.state = "stale";
    return copy;
}

function read(reader, event) {
    if (event.type === "start" && reader.state === "idle") return {state: "reading", flight: event.flight, live: reader.live};
    if (reader.state !== "reading") return reader;
    if (event.type === "settle" && event.flight === reader.flight) return {state: "idle", live: event.data || offline(reader.live)};
    if (event.type === "timeout") return {state: "idle", live: offline(reader.live)};
    return reader;
}

function send(sender, event) {
    if (event.type === "start" && sender.state !== "sending") return {state: "sending", flight: event.flight, command: event.command};
    if (event.type === "feedbackElapsed" && sender.state === "accepted") return UNSENT;
    if (sender.state !== "sending") return sender;
    if (event.type === "settle" && event.flight === sender.flight) return {state: event.accepted ? "accepted" : "uncertain", command: sender.command};
    if (event.type === "timeout") return {state: "uncertain", command: sender.command};
    return sender;
}

function hold(press, event) {
    function to(next, effects) { return {press: next, effects: effects}; }
    switch (event.type) {
    case "press":
        if (press.state === "idle") return to({state: "pressed"}, ["startHoldTimer"]);
        if (press.state === "releasing") return to({state: press.from}, ["stopReleaseTimer"]);
        break;
    case "release":
        if (press.state === "pressed" || press.state === "held") return to({state: "releasing", from: press.state}, ["startReleaseTimer"]);
        if (press.state === "releasing") return to(press, ["startReleaseTimer"]);
        break;
    case "holdElapsed":
        if (press.state !== "pressed" && press.from !== "pressed") break;
        if (event.editable) return to(RELEASED, ["stopTimers", "edit"]);
        return to(press.state === "pressed" ? {state: "held"} : {state: "releasing", from: "held"}, []);
    case "releaseElapsed":
        if (press.state === "releasing") return to(RELEASED, ["stopTimers", "activate"]);
        break;
    case "cancel":
        return to(RELEASED, ["stopTimers"]);
    }
    return to(press, []);
}
