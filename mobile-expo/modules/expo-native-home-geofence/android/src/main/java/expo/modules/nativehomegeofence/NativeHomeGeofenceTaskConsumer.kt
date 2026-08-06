package expo.modules.nativehomegeofence

import android.Manifest
import android.app.PendingIntent
import android.app.job.JobParameters
import android.app.job.JobService
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.location.LocationManager
import android.os.Build
import android.os.Bundle
import android.os.PersistableBundle
import androidx.core.content.ContextCompat
import expo.modules.interfaces.taskManager.TaskConsumer
import expo.modules.interfaces.taskManager.TaskInterface
import expo.modules.interfaces.taskManager.TaskManagerUtilsInterface

class NativeHomeGeofenceTaskConsumer(
  context: Context,
  taskManagerUtils: TaskManagerUtilsInterface,
) : TaskConsumer(context, taskManagerUtils) {
  private var task: TaskInterface? = null
  private var pendingIntent: PendingIntent? = null

  override fun taskType() = "native-home-geofence"

  override fun didRegister(task: TaskInterface) {
    this.task = task
    startMonitoring()
  }

  override fun didUnregister() {
    stopMonitoring()
    task = null
  }

  override fun setOptions(options: Map<String, Any>) {
    stopMonitoring()
    startMonitoring()
  }

  override fun didReceiveBroadcast(intent: Intent) {
    if (intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
      startMonitoring()
      return
    }
    if (intent.getBooleanExtra(LocationManager.KEY_PROXIMITY_ENTERING, true)) {
      return
    }
    val currentTask = task ?: return
    val data = PersistableBundle().apply {
      putInt("eventType", GEOFENCE_EVENT_EXIT)
      putPersistableBundle("region", regionBundle(currentTask.options))
    }
    taskManagerUtils.scheduleJob(requireNotNull(context), currentTask, listOf(data))
  }

  override fun didExecuteJob(jobService: JobService, params: JobParameters): Boolean {
    val currentTask = task ?: return false
    for (item in taskManagerUtils.extractDataFromJobParams(params)) {
      val data = Bundle().apply {
        putInt("eventType", item.getInt("eventType"))
        putBundle("region", Bundle().apply { putAll(item.getPersistableBundle("region")) })
      }
      currentTask.execute(data, null) { jobService.jobFinished(params, false) }
    }
    return true
  }

  override fun canReceiveCustomBroadcast(action: String) =
    action == Intent.ACTION_BOOT_COMPLETED || action == Intent.ACTION_MY_PACKAGE_REPLACED

  private fun startMonitoring() {
    val currentTask = task ?: return
    val currentContext = context ?: return
    val hasFineLocation = ContextCompat.checkSelfPermission(currentContext, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED
    val hasBackgroundLocation = Build.VERSION.SDK_INT < Build.VERSION_CODES.Q ||
      ContextCompat.checkSelfPermission(currentContext, Manifest.permission.ACCESS_BACKGROUND_LOCATION) == PackageManager.PERMISSION_GRANTED
    if (!hasFineLocation || !hasBackgroundLocation) {
      return
    }
    val options = currentTask.options
    val intent = taskManagerUtils.createTaskIntent(currentContext, currentTask)
    val locationManager = currentContext.getSystemService(Context.LOCATION_SERVICE) as LocationManager
    @Suppress("DEPRECATION")
    locationManager.addProximityAlert(
      number(options, "latitude"),
      number(options, "longitude"),
      number(options, "radiusMeters").toFloat(),
      -1L,
      intent,
    )
    pendingIntent = intent
  }

  private fun stopMonitoring() {
    val currentContext = context ?: return
    pendingIntent?.let { intent ->
      val locationManager = currentContext.getSystemService(Context.LOCATION_SERVICE) as LocationManager
      try {
        @Suppress("DEPRECATION")
        locationManager.removeProximityAlert(intent)
      } catch (_: SecurityException) {
        // Permission revocation must not prevent Expo TaskManager from forgetting the task.
      }
      intent.cancel()
    }
    pendingIntent = null
  }

  private fun regionBundle(options: Map<String, Any>) = PersistableBundle().apply {
    putString("identifier", options["identifier"] as String)
    putDouble("latitude", number(options, "latitude"))
    putDouble("longitude", number(options, "longitude"))
    putDouble("radius", number(options, "radiusMeters"))
    putInt("state", GEOFENCE_STATE_OUTSIDE)
  }

  private fun number(options: Map<String, Any>, key: String): Double =
    (options[key] as? Number)?.toDouble()
      ?: throw IllegalArgumentException("$key must be a number")

  private companion object {
    const val GEOFENCE_EVENT_EXIT = 2
    const val GEOFENCE_STATE_OUTSIDE = 2
  }
}
