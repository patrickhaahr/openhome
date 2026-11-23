package com.patrickhaahr.openhome.fitness

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.database.SQLException
import android.media.AudioAttributes
import android.os.Build
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.app.AlarmManagerCompat
import com.patrickhaahr.openhome.OpenHomeApplication
import kotlinx.coroutines.launch

private val VIBRATION_PATTERN = longArrayOf(0, 400, 200, 400)
private const val NO_REPEAT = -1
private const val LOG_TAG = "RestAlarm"

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
                if (container.restTimers.onAlarm()) {
                    val vibration = RestVibration.play(context)
                    if (vibration is RestVibration.Outcome.Failed) Log.w(LOG_TAG, vibration.reason)
                }
            } catch (failure: SQLException) {
                Log.e(LOG_TAG, "Rest alarm could not read or update the rest timer", failure)
            } finally {
                delivery.finish()
            }
        }
    }
}

object RestVibration {
    sealed interface Outcome {
        data object Played : Outcome

        data class Failed(val reason: String) : Outcome
    }

    fun play(context: Context): Outcome {
        val vibrator = vibratorOf(context) ?: return Outcome.Failed("Device has no vibrator service")
        if (!vibrator.hasVibrator()) return Outcome.Failed("Device has no vibrator")
        return try {
            vibrate(vibrator)
            Outcome.Played
        } catch (failure: SecurityException) {
            Outcome.Failed("Vibration refused: ${failure.message}")
        }
    }

    private fun vibratorOf(context: Context): Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getSystemService(VibratorManager::class.java)?.defaultVibrator
        } else {
            context.getSystemService(Vibrator::class.java)
        }

    private fun vibrate(vibrator: Vibrator) {
        when {
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU -> vibrateWithAttributes(vibrator)
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.O -> vibrateWithAudioAttributes(vibrator)
            else -> vibrateWithPattern(vibrator)
        }
    }

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    private fun vibrateWithAttributes(vibrator: Vibrator) {
        val effect = VibrationEffect.createWaveform(VIBRATION_PATTERN, NO_REPEAT)
        vibrator.vibrate(effect, VibrationAttributes.createForUsage(VibrationAttributes.USAGE_ALARM))
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun vibrateWithAudioAttributes(vibrator: Vibrator) {
        val effect = VibrationEffect.createWaveform(VIBRATION_PATTERN, NO_REPEAT)
        vibrator.vibrate(effect, alarmAudioAttributes())
    }

    private fun vibrateWithPattern(vibrator: Vibrator) {
        vibrator.vibrate(VIBRATION_PATTERN, NO_REPEAT, alarmAudioAttributes())
    }

    private fun alarmAudioAttributes(): AudioAttributes =
        AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).build()
}
