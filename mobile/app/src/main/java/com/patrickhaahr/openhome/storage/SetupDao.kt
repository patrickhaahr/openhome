package com.patrickhaahr.openhome.storage

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Entity
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Upsert
import kotlinx.coroutines.flow.Flow

enum class SetupStatus {
    DOWNLOADING,

    COMPLETE,
}

@Entity(tableName = "setup_state")
class SetupStateEntity(
    @PrimaryKey val id: Int = SINGLE_ROW_ID,
    val status: SetupStatus,
    @ColumnInfo(name = "base_url") val baseUrl: String,
    @ColumnInfo(name = "api_key_ciphertext", typeAffinity = ColumnInfo.BLOB) val apiKeyCiphertext: ByteArray?,
    @ColumnInfo(name = "api_key_iv", typeAffinity = ColumnInfo.BLOB) val apiKeyIv: ByteArray?,
)

@Dao
interface SetupDao {
    @Query("SELECT * FROM setup_state WHERE id = $SINGLE_ROW_ID")
    fun observe(): Flow<SetupStateEntity?>

    @Upsert
    suspend fun save(state: SetupStateEntity)
}
