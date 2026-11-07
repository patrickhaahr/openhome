package com.patrickhaahr.openhome.storage

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Entity
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Upsert
import kotlinx.coroutines.flow.Flow

enum class SetupStatus {
    /** A download began and has not finished; nothing from it has been saved. */
    DOWNLOADING,

    /** The initial download committed; the app no longer needs the Setup Flow. */
    COMPLETE,
}

/** Holds the Base URL and the API Key sealed by the Android Keystore; the key is never plain text here. */
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
