package com.solstice.dispensary.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.solstice.dispensary.data.dao.CartDao
import com.solstice.dispensary.data.dao.CustomerDao
import com.solstice.dispensary.data.dao.InventoryDao
import com.solstice.dispensary.data.dao.OrderDao
import com.solstice.dispensary.data.dao.ProductDao
import com.solstice.dispensary.data.dao.ProductRequestDao
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.Customer
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ProductRequest
import com.solstice.dispensary.data.model.ProductSize
import com.solstice.dispensary.data.model.SizePricing
import com.solstice.dispensary.data.model.StrainType

class Converters {
    @TypeConverter
    fun fromCategory(value: ProductCategory): String = value.name

    @TypeConverter
    fun toCategory(value: String): ProductCategory = ProductCategory.valueOf(value)

    @TypeConverter
    fun fromStrain(value: StrainType): String = value.name

    @TypeConverter
    fun toStrain(value: String): StrainType = StrainType.valueOf(value)

    @TypeConverter
    fun fromRole(value: AccountRole): String = value.name

    @TypeConverter
    fun toRole(value: String): AccountRole = AccountRole.valueOf(value)
}

@Database(
    entities = [
        Product::class,
        CartItem::class,
        Order::class,
        OrderLine::class,
        InventoryIntake::class,
        Customer::class,
        ProductRequest::class
    ],
    version = 14,
    exportSchema = false
)
@TypeConverters(Converters::class)
abstract class DispensaryDatabase : RoomDatabase() {
    abstract fun productDao(): ProductDao
    abstract fun cartDao(): CartDao
    abstract fun orderDao(): OrderDao
    abstract fun inventoryDao(): InventoryDao
    abstract fun customerDao(): CustomerDao
    abstract fun productRequestDao(): ProductRequestDao

    companion object {
        @Volatile
        private var instance: DispensaryDatabase? = null

        fun get(context: Context): DispensaryDatabase {
            return instance ?: synchronized(this) {
                instance ?: Room.databaseBuilder(
                    context.applicationContext,
                    DispensaryDatabase::class.java,
                    "solstice_dispensary.db"
                )
                    .fallbackToDestructiveMigration()
                    .build()
                    .also { instance = it }
            }
        }
    }
}

object SeedCatalog {
    private fun flowerSizes(eighthPrice: Double, stockGram: Int, stockEighth: Int, stockQuarter: Int, stockOunce: Int): Product.() -> Product {
        val prices = SizePricing.fromEighth(eighthPrice)
        return {
            copy(
                sizeInventoryEnabled = true,
                unitLabel = "1g–1oz",
                price = eighthPrice,
                priceGram = prices.getValue(ProductSize.GRAM),
                priceEighth = prices.getValue(ProductSize.EIGHTH),
                priceQuarter = prices.getValue(ProductSize.QUARTER),
                priceOunce = prices.getValue(ProductSize.OUNCE),
                stockGram = stockGram,
                stockEighth = stockEighth,
                stockQuarter = stockQuarter,
                stockOunce = stockOunce
            ).normalizedSizeInventory()
        }
    }

    val products = listOf(
        Product(
            id = "fl-dusk",
            name = "Dusk Bloom",
            brand = "Solstice Farms",
            category = ProductCategory.FLOWER,
            strainType = StrainType.INDICA,
            thcPercent = 24.5,
            cbdPercent = 0.4,
            price = 45.0,
            unitLabel = "3.5g",
            description = "Dense purple-tipped buds with berry and cedar notes. Evening wind-down favorite.",
            effects = "Relaxed · Sleepy · Calm",
            featured = true,
            stockQuantity = 24,
            sku = "SOL-FL-DUSK",
            onDeal = true,
            dealPercent = 15,
            dealLabel = "Evening special"
        ).let(flowerSizes(45.0, stockGram = 12, stockEighth = 24, stockQuarter = 10, stockOunce = 4)),
        Product(
            id = "fl-citrus",
            name = "Citrus Ridge",
            brand = "Solstice Farms",
            category = ProductCategory.FLOWER,
            strainType = StrainType.SATIVA,
            thcPercent = 21.2,
            cbdPercent = 0.3,
            price = 42.0,
            unitLabel = "3.5g",
            description = "Bright zest and pine. Clean daytime energy without the jitters.",
            effects = "Uplifted · Focused · Creative",
            featured = true,
            stockQuantity = 18,
            sku = "SOL-FL-CITR"
        ).let(flowerSizes(42.0, stockGram = 10, stockEighth = 18, stockQuarter = 8, stockOunce = 3)),
        Product(
            id = "fl-harbor",
            name = "Harbor Hybrid",
            brand = "North Reach",
            category = ProductCategory.FLOWER,
            strainType = StrainType.HYBRID,
            thcPercent = 22.8,
            cbdPercent = 0.5,
            price = 48.0,
            unitLabel = "3.5g",
            description = "Balanced body ease with a clear head. Great all-rounder.",
            effects = "Balanced · Happy · Mellow",
            stockQuantity = 12,
            sku = "NR-FL-HARB"
        ).let(flowerSizes(48.0, stockGram = 8, stockEighth = 12, stockQuarter = 6, stockOunce = 2)),
        Product(
            id = "pr-twilight",
            name = "Twilight Pack",
            brand = "Solstice Farms",
            category = ProductCategory.PREROLL,
            strainType = StrainType.INDICA,
            thcPercent = 23.0,
            cbdPercent = 0.4,
            price = 28.0,
            unitLabel = "5-pack",
            description = "Ready-to-light evening pre-rolls packed with Dusk Bloom flower.",
            effects = "Relaxed · Sleepy",
            featured = true,
            stockQuantity = 30,
            sku = "SOL-PR-TWIL",
            onDeal = true,
            dealPercent = 20,
            dealLabel = "Pre-roll pack deal"
        ),
        Product(
            id = "pr-spark",
            name = "Spark Singles",
            brand = "North Reach",
            category = ProductCategory.PREROLL,
            strainType = StrainType.SATIVA,
            thcPercent = 20.5,
            cbdPercent = 0.2,
            price = 12.0,
            unitLabel = "1g",
            description = "Single sativa pre-roll for a quick lift on the go.",
            effects = "Energetic · Social",
            stockQuantity = 40,
            sku = "NR-PR-SPARK"
        ),
        Product(
            id = "ed-cocoa",
            name = "Midnight Cocoa",
            brand = "Hearth Kitchen",
            category = ProductCategory.EDIBLE,
            strainType = StrainType.HYBRID,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 24.0,
            unitLabel = "10x10mg",
            description = "Dark chocolate squares with a slow, steady onset. Dose clearly marked.",
            effects = "Relaxed · Euphoric",
            featured = true,
            stockQuantity = 22,
            sku = "HK-ED-COCO",
            onDeal = true,
            dealPercent = 10,
            dealLabel = "Edible of the week"
        ),
        Product(
            id = "ed-gummy",
            name = "Grove Gummies",
            brand = "Hearth Kitchen",
            category = ProductCategory.EDIBLE,
            strainType = StrainType.HYBRID,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 22.0,
            unitLabel = "10x10mg",
            description = "Berry-citrus gummies. Precise 10mg servings for easy pacing.",
            effects = "Happy · Calm",
            stockQuantity = 28,
            sku = "HK-ED-GUM"
        ),
        Product(
            id = "ed-cbd",
            name = "Soft Day CBD",
            brand = "Lumen Lab",
            category = ProductCategory.EDIBLE,
            strainType = StrainType.CBD,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 26.0,
            unitLabel = "20x25mg CBD",
            description = "CBD softgels for daytime calm without intoxication.",
            effects = "Calm · Clear",
            stockQuantity = 16,
            sku = "LL-ED-SOFT"
        ),
        Product(
            id = "co-live",
            name = "Live Rosin Drop",
            brand = "Solstice Farms",
            category = ProductCategory.CONCENTRATE,
            strainType = StrainType.HYBRID,
            thcPercent = 72.0,
            cbdPercent = 1.2,
            price = 55.0,
            unitLabel = "1g",
            description = "Cold-cured live rosin with full-spectrum flavor and potency.",
            effects = "Intense · Euphoric",
            featured = true,
            stockQuantity = 8,
            sku = "SOL-CO-LIVE"
        ),
        Product(
            id = "co-shatter",
            name = "Amber Shatter",
            brand = "North Reach",
            category = ProductCategory.CONCENTRATE,
            strainType = StrainType.INDICA,
            thcPercent = 78.5,
            cbdPercent = 0.8,
            price = 40.0,
            unitLabel = "1g",
            description = "Classic shatter for experienced consumers. Strong and clean.",
            effects = "Heavy · Relaxed",
            stockQuantity = 10,
            sku = "NR-CO-AMBR"
        ),
        Product(
            id = "vp-cart",
            name = "Solstice Cart — Sage",
            brand = "Solstice Farms",
            category = ProductCategory.VAPE,
            strainType = StrainType.HYBRID,
            thcPercent = 85.0,
            cbdPercent = 0.5,
            price = 38.0,
            unitLabel = "0.5g",
            description = "Ceramic cart with herbal and citrus terps. Discreet and consistent.",
            effects = "Balanced · Smooth",
            featured = true,
            stockQuantity = 14,
            sku = "SOL-VP-SAGE",
            onDeal = true,
            dealPercent = 25,
            dealLabel = "Cart clearance"
        ),
        Product(
            id = "vp-disp",
            name = "Pocket Disposable",
            brand = "Lumen Lab",
            category = ProductCategory.VAPE,
            strainType = StrainType.SATIVA,
            thcPercent = 82.0,
            cbdPercent = 0.3,
            price = 32.0,
            unitLabel = "0.3g",
            description = "Draw-activated disposable. No charging, no fuss.",
            effects = "Uplifted · Creative",
            stockQuantity = 20,
            sku = "LL-VP-POCK"
        ),
        Product(
            id = "tp-balm",
            name = "Trail Balm",
            brand = "Hearth Kitchen",
            category = ProductCategory.TOPICAL,
            strainType = StrainType.CBD,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 30.0,
            unitLabel = "2oz",
            description = "Menthol CBD balm for sore muscles after a long day.",
            effects = "Soothing · Localized",
            stockQuantity = 11,
            sku = "HK-TP-TRAL"
        ),
        Product(
            id = "tp-lotion",
            name = "Evening Lotion",
            brand = "Lumen Lab",
            category = ProductCategory.TOPICAL,
            strainType = StrainType.CBD,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 34.0,
            unitLabel = "4oz",
            description = "Lavender CBD body lotion — non-intoxicating daily care.",
            effects = "Calm · Comfort",
            stockQuantity = 9,
            sku = "LL-TP-EVEN"
        ),
        Product(
            id = "ac-grinder",
            name = "Four-Piece Grinder",
            brand = "Solstice Gear",
            category = ProductCategory.ACCESSORY,
            strainType = StrainType.NONE,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 18.0,
            unitLabel = "each",
            description = "Anodized aluminum grinder with pollen catcher.",
            effects = "—",
            stockQuantity = 25,
            sku = "SG-AC-GRND"
        ),
        Product(
            id = "ac-tray",
            name = "Rolling Tray",
            brand = "Solstice Gear",
            category = ProductCategory.ACCESSORY,
            strainType = StrainType.NONE,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 14.0,
            unitLabel = "each",
            description = "Matte charcoal tray with raised rim. Compact travel size.",
            effects = "—",
            stockQuantity = 15,
            sku = "SG-AC-TRAY",
            published = false
        )
    )
}
