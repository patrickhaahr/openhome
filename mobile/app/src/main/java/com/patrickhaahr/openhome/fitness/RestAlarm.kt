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

class RestAlarm(private val context: Context) {
    private val manager = context.getSystemService(AlarmManager::class.java)

    fun setAt(deadlineAt: Long) {
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
            } finally {
                delivery.finish()
            }
        }
    }

    @Suppress("DEPRECATION")
    private fun vibrate(context: Context) {
        val attributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).build()
        context.getSystemService(Vibrator::class.java).vibrate(VIBRATION_PATTERN, NO_REPEAT, attributes)
    }
}
