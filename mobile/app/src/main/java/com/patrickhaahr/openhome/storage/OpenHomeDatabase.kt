package com.patrickhaahr.openhome.storage

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

/** Add a migration with every schema change; fitness data here may not exist anywhere else yet. */
@Database(
    version = 1,
    entities =
        [
            ExerciseEntity::class,
            WorkoutEntity::class,
            WorkoutEntryEntity::class,
            WorkoutSetEntity::class,
            BodyWeightEntity::class,
            ProfileEntity::class,
            SetupStateEntity::class,
        ],
)
abstract class OpenHomeDatabase : RoomDatabase() {
    abstract fun fitnessDao(): FitnessDao

    abstract fun setupDao(): SetupDao

    companion object {
        fun open(context: Context): OpenHomeDatabase =
            Room.databaseBuilder(context, OpenHomeDatabase::class.java, "openhome.db").build()
    }
}
