package com.patrickhaahr.openhome.sync

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.distinctUntilChanged

class WifiNetworks(context: Context) {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)

    fun activeWifiNetwork(): Network? =
        connectivity.activeNetwork?.takeIf { network ->
            connectivity.getNetworkCapabilities(network)?.isWifi() == true
        }

    fun observeOnWifi(): Flow<Boolean> =
        callbackFlow {
            val callback =
                object : ConnectivityManager.NetworkCallback() {
                    override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
                        trySend(capabilities.isWifi())
                    }

                    override fun onLost(network: Network) {
                        trySend(false)
                    }
                }
            trySend(activeWifiNetwork() != null)
            connectivity.registerDefaultNetworkCallback(callback)
            awaitClose { connectivity.unregisterNetworkCallback(callback) }
        }.distinctUntilChanged()

    private fun NetworkCapabilities.isWifi() = hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
}
