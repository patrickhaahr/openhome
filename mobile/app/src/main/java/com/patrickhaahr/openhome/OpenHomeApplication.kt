package com.patrickhaahr.openhome

import android.app.Application

class OpenHomeApplication : Application() {
    val container by lazy { AppContainer(this) }
}
