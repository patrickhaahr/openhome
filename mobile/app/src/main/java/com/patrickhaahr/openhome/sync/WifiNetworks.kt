package com.patrickhaahr.openhome.sync

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * The fitness sync policy: the app's default network must run over Wi-Fi. It looks only at the
 * transport, so mobile data and any other connection stay out however unmetered or reachable they
 * are. A VPN over Wi-Fi reports the Wi-Fi transport too and passes; a VPN over mobile data does not.
 */
class WifiNetworks(context: Context) {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)

    /** The network to send fitness changes over, or null when policy forbids sending now. */
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
