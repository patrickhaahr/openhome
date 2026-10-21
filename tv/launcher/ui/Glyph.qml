import QtQuick 2.12

Canvas {
    property string kind: "bulb"
    property color ink: "#ecece6"
    onKindChanged: requestPaint()
    onInkChanged: requestPaint()
    onPaint: {
        var c = getContext("2d");
        c.reset(); c.scale(width / 48, height / 48);
        c.strokeStyle = ink; c.lineWidth = 2.2; c.lineCap = "round"; c.lineJoin = "round";
        c.beginPath();
        if (kind === "bulb" || kind === "bulb-off") {
            c.moveTo(18, 34); c.bezierCurveTo(18, 27, 11, 25, 11, 17);
            c.bezierCurveTo(11, 1, 37, 1, 37, 17); c.bezierCurveTo(37, 25, 30, 27, 30, 34);
            c.closePath(); c.moveTo(19, 40); c.lineTo(29, 40); c.moveTo(22, 45); c.lineTo(26, 45);
            if (kind === "bulb-off") { c.moveTo(5, 4); c.lineTo(43, 43); }
        } else if (kind === "bluetooth") {
            c.moveTo(14, 12); c.lineTo(34, 32); c.lineTo(24, 42); c.lineTo(24, 6);
            c.lineTo(34, 16); c.lineTo(14, 36);
        } else if (kind === "optical") {
            c.rect(10, 8, 28, 31); c.moveTo(18, 39); c.lineTo(18, 45);
            c.moveTo(30, 39); c.lineTo(30, 45); c.moveTo(17, 20); c.lineTo(24, 27); c.lineTo(31, 20);
        } else if (kind === "charge") {
            c.moveTo(28, 4); c.lineTo(12, 27); c.lineTo(24, 27); c.lineTo(20, 44);
            c.lineTo(37, 20); c.lineTo(25, 20); c.closePath();
        } else if (kind === "rest") {
            c.arc(25, 23, 18, 0.2, 4.5); c.bezierCurveTo(13, 24, 25, 36, 42, 27);
        } else if (kind === "effort") {
            c.moveTo(3, 25); c.lineTo(13, 25); c.lineTo(20, 10); c.lineTo(28, 39);
            c.lineTo(35, 25); c.lineTo(45, 25);
        } else if (kind === "sun") {
            c.arc(24, 24, 10, 0, Math.PI * 2);
            for (var i = 0; i < 8; i++) {
                var a = i * Math.PI / 4;
                c.moveTo(24 + 16 * Math.cos(a), 24 + 16 * Math.sin(a));
                c.lineTo(24 + 22 * Math.cos(a), 24 + 22 * Math.sin(a));
            }
        } else if (kind === "rain") {
            c.moveTo(8, 28); c.bezierCurveTo(-1, 19, 10, 11, 17, 15);
            c.bezierCurveTo(19, -1, 41, 3, 40, 18); c.bezierCurveTo(50, 20, 45, 32, 36, 30);
            c.lineTo(8, 30); c.moveTo(15, 37); c.lineTo(12, 44);
            c.moveTo(26, 37); c.lineTo(23, 44); c.moveTo(37, 37); c.lineTo(34, 44);
        } else if (kind === "snow") {
            for (var j = 0; j < 3; j++) {
                var b = j * Math.PI / 3;
                c.moveTo(24 - 20 * Math.cos(b), 24 - 20 * Math.sin(b));
                c.lineTo(24 + 20 * Math.cos(b), 24 + 20 * Math.sin(b));
            }
        } else if (kind === "cloud") {
            c.moveTo(9, 35); c.bezierCurveTo(-3, 31, 1, 16, 13, 18);
            c.bezierCurveTo(15, -1, 43, 5, 39, 23); c.bezierCurveTo(51, 24, 47, 37, 38, 35); c.closePath();
        } else {
            c.moveTo(12, 20); c.lineTo(24, 32); c.lineTo(36, 20);
        }
        c.stroke();
    }
}
