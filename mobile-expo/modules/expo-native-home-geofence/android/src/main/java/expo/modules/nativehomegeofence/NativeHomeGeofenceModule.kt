package expo.modules.nativehomegeofence

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Build
import android.os.CancellationSignal
import android.os.Looper
import androidx.core.content.ContextCompat
import expo.modules.interfaces.taskManager.TaskManagerInterface
import expo.modules.kotlin.functions.Coroutine
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume

class NativeHomeGeofenceModule : Module() {
  private val context: Context
    get() = requireNotNull(appContext.reactContext?.applicationContext) { "Android application context is unavailable" }

  private val locationManager: LocationManager
    get() = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager

  private val taskManager: TaskManagerInterface
    get() = requireNotNull(appContext.legacyModule<TaskManagerInterface>()) { "Expo TaskManager is unavailable" }

  override fun definition() = ModuleDefinition {
    Name("ExpoNativeHomeGeofence")

    AsyncFunction("getCurrentPositionAsync") Coroutine { ->
      checkFineLocationPermission()
      val location = currentLocation()
        ?: throw IllegalStateException("No Android location provider could determine the current location")
      mapOf(
        "latitude" to location.latitude,
        "longitude" to location.longitude,
        "accuracyMeters" to if (location.hasAccuracy()) location.accuracy.toDouble() else null,
      )
    }

    AsyncFunction("startMonitoringAsync") { taskName: String, home: Map<String, Any> ->
      checkFineLocationPermission()
      checkBackgroundLocationPermission()
      taskManager.registerTask(taskName, NativeHomeGeofenceTaskConsumer::class.java, home)
    }

    AsyncFunction("stopMonitoringAsync") { taskName: String ->
      if (taskManager.taskHasConsumerOfClass(taskName, NativeHomeGeofenceTaskConsumer::class.java)) {
        taskManager.unregisterTask(taskName, NativeHomeGeofenceTaskConsumer::class.java)
      }
    }

    AsyncFunction("hasStartedMonitoringAsync") { taskName: String ->
      taskManager.taskHasConsumerOfClass(taskName, NativeHomeGeofenceTaskConsumer::class.java)
    }
  }

  private fun checkFineLocationPermission() {
    check(ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED) {
      "Precise location permission is required"
    }
  }

  private fun checkBackgroundLocationPermission() {
    check(
      Build.VERSION.SDK_INT < Build.VERSION_CODES.Q ||
        ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_BACKGROUND_LOCATION) == PackageManager.PERMISSION_GRANTED,
    ) {
      "Background location permission is required"
    }
  }

  private suspend fun currentLocation(): Location? {
    for (provider in preferredProviders()) {
      val location = withTimeoutOrNull(CURRENT_LOCATION_TIMEOUT_MILLIS) {
        currentLocation(provider)
      }
      if (location != null) {
        return location
      }
    }
    return null
  }

  private fun preferredProviders(): List<String> {
    val available = locationManager.getProviders(true).toSet()
    return buildList {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && LocationManager.FUSED_PROVIDER in available) {
        add(LocationManager.FUSED_PROVIDER)
      }
      if (LocationManager.GPS_PROVIDER in available) {
        add(LocationManager.GPS_PROVIDER)
      }
      if (LocationManager.NETWORK_PROVIDER in available) {
        add(LocationManager.NETWORK_PROVIDER)
      }
    }
  }

  private suspend fun currentLocation(provider: String): Location? = suspendCancellableCoroutine { continuation ->
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      val cancellationSignal = CancellationSignal()
      continuation.invokeOnCancellation { cancellationSignal.cancel() }
      locationManager.getCurrentLocation(provider, cancellationSignal, context.mainExecutor) { location ->
        if (continuation.isActive) {
          continuation.resume(location)
        }
      }
      return@suspendCancellableCoroutine
    }

    val listener = object : LocationListener {
      override fun onLocationChanged(location: Location) {
        locationManager.removeUpdates(this)
        if (continuation.isActive) {
          continuation.resume(location)
        }
      }

      @Deprecated("Required on Android versions before API 30")
      override fun onStatusChanged(provider: String?, status: Int, extras: android.os.Bundle?) = Unit
    }
    continuation.invokeOnCancellation { locationManager.removeUpdates(listener) }
    @Suppress("DEPRECATION")
    locationManager.requestSingleUpdate(provider, listener, Looper.getMainLooper())
  }

  private companion object {
    const val CURRENT_LOCATION_TIMEOUT_MILLIS = 15_000L
  }
}
