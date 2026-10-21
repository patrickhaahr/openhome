import QtQuick 2.12

Image {
    property string kind: "bulb"
    property color ink: "#ecece6"
    source: "file:///var/lib/pearwebos/ui/icons/" + kind + "-" + String(ink).replace("#", "") + ".png"
    fillMode: Image.PreserveAspectFit
    smooth: true
    mipmap: true
}
