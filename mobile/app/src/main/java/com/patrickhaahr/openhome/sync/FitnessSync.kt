package com.patrickhaahr.openhome.sync

import android.content.Context
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.patrickhaahr.openhome.OpenHomeApplication
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.storage.OutboxDao
import com.patrickhaahr.openhome.storage.SyncProblem
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import okhttp3.Dns
import okhttp3.OkHttpClient

private const val WORK_NAME = "fitness-sync"
private const val RETRY_BACKOFF_SECONDS = 30L

class FitnessSync(
    private val outbox: FitnessOutbox,
    private val setup: SetupService,
    private val wifi: WifiNetworks,
    private val httpClient: OkHttpClient,
) {
    private val passes = Mutex()
    private val sendingNow = MutableStateFlow<Long?>(null)

    val sending: StateFlow<Long?> = sendingNow

    suspend fun run(): DeliveryRun =
        passes.withLock {
            if (!outbox.hasPending()) return DeliveryRun.FINISHED
            val network = wifi.activeWifiNetwork() ?: return DeliveryRun.RETRY_LATER
            val connection = setup.connection()
            if (connection == null) {
                outbox.recordProblemOnPending(SyncProblem.API_KEY_UNAVAILABLE)
                return DeliveryRun.RETRY_LATER
            }
            val api = OpenHomeApi(connection.baseUrl, connection.apiKey, httpClient.boundTo(network))
            try {
                outbox.deliverPending(api) { sendingNow.value = it }
            } finally {
                sendingNow.value = null
            }
        }

    private fun OkHttpClient.boundTo(network: Network): OkHttpClient =
        newBuilder()
            .socketFactory(network.socketFactory)
            .dns(Dns { host -> network.getAllByName(host).toList() })
            .build()
}

class FitnessSyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result =
        when ((applicationContext as OpenHomeApplication).container.sync.run()) {
            DeliveryRun.FINISHED -> Result.success()
            DeliveryRun.RETRY_LATER -> Result.retry()
        }
}

class FitnessSyncScheduler(context: Context, private val outbox: OutboxDao) {
    private val workManager by lazy { WorkManager.getInstance(context) }

    fun schedule() {
        val wifiRequest =
            NetworkRequest.Builder()
                .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .removeCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                .build()
        val request =
            OneTimeWorkRequestBuilder<FitnessSyncWorker>()
                .setConstraints(Constraints.Builder().setRequiredNetworkRequest(wifiRequest, NetworkType.UNMETERED).build())
                .setBackoffCriteria(BackoffPolicy.LINEAR, RETRY_BACKOFF_SECONDS, TimeUnit.SECONDS)
                .build()
        workManager.enqueueUniqueWork(WORK_NAME, ExistingWorkPolicy.REPLACE, request)
    }

    suspend fun scheduleIfPending() {
        if (outbox.pendingCount() > 0) schedule()
    }
}
