package com.patrickhaahr.openhome.fitness

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.database.SQLException
import android.media.AudioAttributes
import android.os.Build
import android.os.Vibrator
import androidx.core.app.AlarmManagerCompat
import com.patrickhaahr.openhome.OpenHomeApplication
import kotlinx.coroutines.launch

private val VIBRATION_PATTERN = longArrayOf(0, 400, 200, 400)
private const val NO_REPEAT = -1

/** One alarm for the end of the rest, kept by Android so it fires with no process or screen alive. */
class RestAlarm(private val context: Context) {
    private val manager = context.getSystemService(AlarmManager::class.java)

    /** Replaces any earlier alarm. A deadline already past fires at once. */
    fun setAt(deadlineAt: Long) {
        // A rest ends on time only with an exact alarm; if the user revoked that, late is still better than never.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || manager.canScheduleExactAlarms()) {
            AlarmManagerCompat.setExactAndAllowWhileIdle(manager, AlarmManager.RTC_WAKEUP, deadlineAt, pendingIntent())
        } else {
            AlarmManagerCompat.setAndAllowWhileIdle(manager, AlarmManager.RTC_WAKEUP, deadlineAt, pendingIntent())
        }
    }

    fun cancel() {
        manager.cancel(pendingIntent())
    }

    private fun pendingIntent(): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            0,
            Intent(context, RestAlarmReceiver::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
}

class RestAlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val container = (context.applicationContext as OpenHomeApplication).container
        val delivery = goAsync()
        container.saveScope.launch {
            try {
                if (container.restTimers.onAlarm()) vibrate(context)
            } catch (_: SQLException) {
                // The screen still shows the rest as over; only this vibration is lost.
            } finally {
                delivery.finish()
            }
        }
    }

    /** As an alarm, the kind of vibration Android still lets through while the app is in the background. */
    @Suppress("DEPRECATION") // The one call that exists on every Android version this app supports.
    private fun vibrate(context: Context) {
        val attributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).build()
        context.getSystemService(Vibrator::class.java).vibrate(VIBRATION_PATTERN, NO_REPEAT, attributes)
    }
}
