package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader

@Composable
fun HomeScreen(onOpenProfile: () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        ScreenHeader(title = "Home", onOpenProfile = onOpenProfile)
        EmptyState(
            title = "No Home Controls yet",
            message = "TV, Speaker, Lights and Backlight controls aren't in this version of the app.",
        )
    }
}
