import QtQuick 2.12
import QtGraphicalEffects 1.12

Item {
    id: artwork
    property url source
    property real cornerRadius: 22
    property real iconSize: 154
    property bool opaque: false
    property color ground: "#111213"
    onSourceChanged: { opaque = false; ground = "#111213"; if (source) probe.loadImage(source); }
    // Probe only the image edges; transparent logos retain their original sizing.
    Canvas {
        id: probe; width: 16; height: 16; opacity: 0
        onAvailableChanged: if (available && artwork.source) loadImage(artwork.source)
        onImageLoaded: requestPaint()
        onPaint: {
            if (!isImageLoaded(artwork.source)) return;
            var c = getContext("2d"); c.clearRect(0, 0, 16, 16);
            c.drawImage(artwork.source, 0, 0, 16, 16);
            var top = c.getImageData(8, 0, 1, 1).data;
            var left = c.getImageData(0, 8, 1, 1).data;
            var right = c.getImageData(15, 8, 1, 1).data;
            var bottom = c.getImageData(8, 15, 1, 1).data;
            artwork.opaque = top[3] > 250 && left[3] > 250 && right[3] > 250 && bottom[3] > 250;
            if (artwork.opaque) artwork.ground = Qt.rgba(top[0] / 255, top[1] / 255, top[2] / 255, 1);
        }
    }
    Item {
        id: content; anchors.fill: parent
        layer.enabled: true
        layer.effect: OpacityMask {
            maskSource: Rectangle { width: artwork.width; height: artwork.height; radius: artwork.cornerRadius }
        }
        Rectangle { anchors.fill: parent; color: artwork.ground }
        Image {
            anchors.centerIn: parent
            width: artwork.opaque ? parent.height : artwork.iconSize; height: width
            fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true; asynchronous: true
            source: artwork.source
        }
    }
}
