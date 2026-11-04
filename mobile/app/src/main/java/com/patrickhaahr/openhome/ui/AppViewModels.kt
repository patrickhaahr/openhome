package com.patrickhaahr.openhome.ui

import androidx.compose.runtime.Composable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider.AndroidViewModelFactory.Companion.APPLICATION_KEY
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.patrickhaahr.openhome.AppContainer
import com.patrickhaahr.openhome.OpenHomeApplication

/** A ViewModel scoped to the current destination, built from the app's single instances. */
@Composable
inline fun <reified VM : ViewModel> appViewModel(crossinline create: (AppContainer) -> VM): VM =
    viewModel(
        factory =
            viewModelFactory {
                initializer { create((this[APPLICATION_KEY] as OpenHomeApplication).container) }
            },
    )
