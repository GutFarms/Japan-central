package com.nativepure.companion.data

object SeedCatalog {
    private fun flower(
        product: Product,
        stockGram: Int,
        stockEighth: Int,
        stockQuarter: Int,
        stockOunce: Int
    ): Product {
        val prices = SizePricing.fromEighth(product.price)
        return product.copy(
            sizeInventoryEnabled = true,
            unitLabel = "1g–1oz",
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

    val products = listOf(
        flower(
            Product("fl-dusk", "Dusk Bloom", "Solstice Farms", ProductCategory.FLOWER, StrainType.INDICA, 24.5, 0.4, 45.0, "3.5g", "Dense purple-tipped buds with berry and cedar notes. Evening wind-down favorite.", "Relaxed · Sleepy · Calm", featured = true, stockQuantity = 24, sku = "SOL-FL-DUSK", onDeal = true, dealPercent = 15, dealLabel = "Evening special"),
            stockGram = 12, stockEighth = 24, stockQuarter = 10, stockOunce = 4
        ),
        flower(
            Product("fl-citrus", "Citrus Ridge", "Solstice Farms", ProductCategory.FLOWER, StrainType.SATIVA, 21.2, 0.3, 42.0, "3.5g", "Bright zest and pine. Clean daytime energy without the jitters.", "Uplifted · Focused · Creative", featured = true, stockQuantity = 18, sku = "SOL-FL-CITR"),
            stockGram = 10, stockEighth = 18, stockQuarter = 8, stockOunce = 3
        ),
        flower(
            Product("fl-harbor", "Harbor Hybrid", "North Reach", ProductCategory.FLOWER, StrainType.HYBRID, 22.8, 0.5, 48.0, "3.5g", "Balanced body ease with a clear head. Great all-rounder.", "Balanced · Happy · Mellow", stockQuantity = 12, sku = "NR-FL-HARB"),
            stockGram = 8, stockEighth = 12, stockQuarter = 6, stockOunce = 2
        ),
        Product("pr-twilight", "Twilight Pack", "Solstice Farms", ProductCategory.PREROLL, StrainType.INDICA, 23.0, 0.4, 28.0, "5-pack", "Ready-to-light evening pre-rolls packed with Dusk Bloom flower.", "Relaxed · Sleepy", featured = true, stockQuantity = 30, sku = "SOL-PR-TWIL", onDeal = true, dealPercent = 20, dealLabel = "Pre-roll pack deal"),
        Product("pr-spark", "Spark Singles", "North Reach", ProductCategory.PREROLL, StrainType.SATIVA, 20.5, 0.2, 12.0, "1g", "Single sativa pre-roll for a quick lift on the go.", "Energetic · Social", stockQuantity = 40, sku = "NR-PR-SPARK"),
        Product("ed-cocoa", "Midnight Cocoa", "Hearth Kitchen", ProductCategory.EDIBLE, StrainType.HYBRID, 0.0, 0.0, 24.0, "10x10mg", "Dark chocolate squares with a slow, steady onset. Dose clearly marked.", "Relaxed · Euphoric", featured = true, stockQuantity = 22, sku = "HK-ED-COCO", onDeal = true, dealPercent = 10, dealLabel = "Edible of the week"),
        Product("ed-gummy", "Grove Gummies", "Hearth Kitchen", ProductCategory.EDIBLE, StrainType.HYBRID, 0.0, 0.0, 22.0, "10x10mg", "Berry-citrus gummies. Precise 10mg servings for easy pacing.", "Happy · Calm", stockQuantity = 28, sku = "HK-ED-GUM"),
        Product("ed-cbd", "Soft Day CBD", "Lumen Lab", ProductCategory.EDIBLE, StrainType.CBD, 0.0, 0.0, 26.0, "20x25mg CBD", "CBD softgels for daytime calm without intoxication.", "Calm · Clear", stockQuantity = 16, sku = "LL-ED-SOFT"),
        Product("co-live", "Live Rosin Drop", "Solstice Farms", ProductCategory.CONCENTRATE, StrainType.HYBRID, 72.0, 1.2, 55.0, "1g", "Cold-cured live rosin with full-spectrum flavor and potency.", "Intense · Euphoric", featured = true, stockQuantity = 8, sku = "SOL-CO-LIVE"),
        Product("co-shatter", "Amber Shatter", "North Reach", ProductCategory.CONCENTRATE, StrainType.INDICA, 78.5, 0.8, 40.0, "1g", "Classic shatter for experienced consumers. Strong and clean.", "Heavy · Relaxed", stockQuantity = 10, sku = "NR-CO-AMBR"),
        Product("vp-cart", "Solstice Cart — Sage", "Solstice Farms", ProductCategory.VAPE, StrainType.HYBRID, 85.0, 0.5, 38.0, "0.5g", "Ceramic cart with herbal and citrus terps. Discreet and consistent.", "Balanced · Smooth", featured = true, stockQuantity = 14, sku = "SOL-VP-SAGE", onDeal = true, dealPercent = 25, dealLabel = "Cart clearance"),
        Product("vp-disp", "Pocket Disposable", "Lumen Lab", ProductCategory.VAPE, StrainType.SATIVA, 82.0, 0.3, 32.0, "0.3g", "Draw-activated disposable. No charging, no fuss.", "Uplifted · Creative", stockQuantity = 20, sku = "LL-VP-POCK"),
        Product("tp-balm", "Trail Balm", "Hearth Kitchen", ProductCategory.TOPICAL, StrainType.CBD, 0.0, 0.0, 30.0, "2oz", "Menthol CBD balm for sore muscles after a long day.", "Soothing · Localized", stockQuantity = 11, sku = "HK-TP-TRAL"),
        Product("tp-lotion", "Evening Lotion", "Lumen Lab", ProductCategory.TOPICAL, StrainType.CBD, 0.0, 0.0, 34.0, "4oz", "Lavender CBD body lotion — non-intoxicating daily care.", "Calm · Comfort", stockQuantity = 9, sku = "LL-TP-EVEN"),
        Product("ac-grinder", "Four-Piece Grinder", "Solstice Gear", ProductCategory.ACCESSORY, StrainType.NONE, 0.0, 0.0, 18.0, "each", "Anodized aluminum grinder with pollen catcher.", "—", stockQuantity = 25, sku = "SG-AC-GRND"),
        Product("ac-tray", "Rolling Tray", "Solstice Gear", ProductCategory.ACCESSORY, StrainType.NONE, 0.0, 0.0, 14.0, "each", "Matte charcoal tray with raised rim. Compact travel size.", "—", stockQuantity = 15, sku = "SG-AC-TRAY", published = false)
    )
}
