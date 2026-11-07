package com.patrickhaahr.openhome.storage

import android.content.Context
import androidx.room.AutoMigration
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

/** Add a migration with every schema change; fitness data here may not exist anywhere else yet. */
@Database(
    version = 6,
    autoMigrations =
        [
            AutoMigration(from = 1, to = 2),
            AutoMigration(from = 2, to = 3),
            AutoMigration(from = 3, to = 4),
            AutoMigration(from = 4, to = 5),
            AutoMigration(from = 5, to = 6),
        ],
    entities =
        [
            ExerciseEntity::class,
            WorkoutEntity::class,
            WorkoutEntryEntity::class,
            WorkoutSetEntity::class,
            BodyWeightEntity::class,
            ProfileEntity::class,
            SetupStateEntity::class,
            ActiveWorkoutEntity::class,
            ActiveEntryEntity::class,
            ActiveSetEntity::class,
            OutboxEntity::class,
            ExerciseRestEntity::class,
            RestTimerEntity::class,
        ],
)
abstract class OpenHomeDatabase : RoomDatabase() {
    abstract fun fitnessDao(): FitnessDao

    abstract fun setupDao(): SetupDao

    abstract fun activeWorkoutDao(): ActiveWorkoutDao

    abstract fun outboxDao(): OutboxDao

    abstract fun restDao(): RestDao

    companion object {
        fun open(context: Context): OpenHomeDatabase =
            Room.databaseBuilder(context, OpenHomeDatabase::class.java, "openhome.db").build()
    }
}
